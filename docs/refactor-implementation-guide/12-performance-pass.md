# 12 — Performance pass on the capture pipeline

**Priority:** Later · **Effort:** Medium · **Estimated PR size:** profile-driven; small targeted patches.

## Goal

After the capture path lives in `CaptureEngine` (step 04) and the runtime
boundary is typed (step 05), profile the capture pipeline and remove the
specific cost centres the report flags. This step is deliberately data-driven —
do not pre-optimise.

## Report finding

> "The capture path updates the external texture, performs Y/U/V rendering,
> reads pixels back from three framebuffers, and then hands buffers to native
> code, while using `Instant.now()` / `Duration.between()` for frame throttling.
> Even if this is currently fast enough, it is high-risk code for UI jank,
> capture jitter, and battery cost, and it deserves isolation behind a dedicated
> capture engine plus benchmark/profiling steps before further feature changes."

— `deep-research-report-3.md`, "Detailed findings".

## Pre-state on `main`

The capture frame loop currently lives inside `MainActivity.java`. After step 04
it lives in `CaptureEngine.kt`. Either way the relevant cost centres are
identical:

1. **GPU readback** — `glReadPixels` (or framebuffer-backed `PBufferSurface`
   readback) for three planes (Y, U, V).
2. **Frame throttling** — `Instant.now()` / `Duration.between()` per frame.
3. **Buffer allocation** — per-frame `ByteBuffer.allocateDirect` (verify by
   grep).
4. **JNI hop** — `nativeProcessFrame` called per frame on the GL thread.
5. **`SurfaceTexture.updateTexImage()` cadence** — bound by the producer.

## Step 12.1 — Establish a benchmark harness

Before changing any line of code, land a microbenchmark that measures end-to-end
frame latency from "callback fired" to "native code returned":

```kotlin
// app/src/androidTest/java/org/fcast/android/sender/capture/CaptureBenchmarkTest.kt
@RunWith(AndroidJUnit4::class)
class CaptureBenchmarkTest {
    @get:Rule val benchmarkRule = BenchmarkRule()

    @Test fun frame_roundtrip() {
        val engine = CaptureEngine(onFrame = { /* no-op */ })
        engine.startSyntheticSource(width = 1920, height = 1080, fps = 30)
        benchmarkRule.measureRepeated {
            engine.pumpOneFrame()
        }
    }
}
```

`androidx.benchmark:benchmark-junit4` provides the rule. The Synthetic source
is a `SurfaceTexture` fed by a procedurally-generated bitmap so the benchmark
does not depend on a real `MediaProjection`.

Capture three baseline numbers on a known device (e.g. Pixel 8):

| Metric                         | Baseline (on `main`)            |
|--------------------------------|---------------------------------|
| Per-frame total latency        | _record value_                  |
| `glReadPixels` time            | _record value_                  |
| GC pressure (allocations)      | _record value_                  |

These numbers go into the PR description.

## Step 12.2 — Replace `Instant.now()` with a monotonic clock

`Instant.now()` walks the system clock and is allocation-heavy. Switch to
`System.nanoTime()`:

```diff
- Instant lastFrameAt = Instant.now();
- if (Duration.between(lastFrameAt, Instant.now()).toMillis() < minIntervalMs) return;
+ long lastFrameNs = System.nanoTime();
+ long nowNs = System.nanoTime();
+ if (nowNs - lastFrameNs < minIntervalNs) return;
```

This is a no-thinking-required win on every Android device.

## Step 12.3 — Recycle frame buffers

If a `ByteBuffer.allocateDirect(...)` lives inside the frame callback (verify
with `rg`), move it to a single allocation owned by the engine:

```kotlin
class CaptureEngine(/* … */) {
    private val yBuf  = ByteBuffer.allocateDirect(MAX_W * MAX_H)
    private val uBuf  = ByteBuffer.allocateDirect(MAX_W * MAX_H / 4)
    private val vBuf  = ByteBuffer.allocateDirect(MAX_W * MAX_H / 4)
    // …
}
```

Re-zero or `clear()` per frame; never `allocate`.

## Step 12.4 — Reduce `glReadPixels` cost

`glReadPixels` of three planes on three FBOs is the headline cost. Two
alternatives:

- **Single packed read.** Render to one RGBA FBO and convert Y/U/V on the CPU
  (faster on many devices; depends on GPU).
- **`HardwareBuffer` / `EGLImage`.** Read once into a `HardwareBuffer`, hand
  the buffer's file descriptor to native code, avoid the GPU→CPU copy entirely.
  This is the right long-term shape but is the bigger change.

Land a benchmark for each option before picking one.

## Step 12.5 — Move the JNI hop off the GL thread

If `nativeProcessFrame` does any work beyond a memcpy + enqueue, it should not
run on the GL thread. Use the existing `glThread` only for GL state, and
forward the captured buffer to a dedicated `HandlerThread("CapturePump")`:

```kotlin
private val pump = HandlerThread("CapturePump").also { it.start() }
private val pumpHandler = Handler(pump.looper)

private fun onFrame(buf: FrameRef) {
    pumpHandler.post { nativeProcessFrame(buf) }
}
```

The buffer ownership rule is: GL thread fills, pump thread consumes; the
buffer is "borrowed" until the native callback returns.

## Step 12.6 — Audit GC pressure

Run the benchmark with `androidx.tracing.Trace` enabled and capture an Android
Studio profiler trace. Targets:

- Zero allocations per frame (`HeapTaskDaemon` flat).
- No per-frame `String.format`, no `Log.d` with formatted args, no per-frame
  `Bitmap.createBitmap`.

## Testing

| Test                                                            | How                                                                      |
|-----------------------------------------------------------------|--------------------------------------------------------------------------|
| Benchmark numbers improved                                       | Re-run `CaptureBenchmarkTest`; baseline numbers must drop or stay flat.  |
| No regression in capture quality                                | Manual A/B against `main` build on the same device.                      |
| No new GC pauses                                                 | Profiler trace.                                                          |
| Slint headless UI tests still pass                              | `cargo test -p fcastsender --test ui_snapshots`.                          |

## Rollback

Performance changes should land as small patches, each with the benchmark delta
in the PR body. Revert any patch whose benchmark delta did not materialise on
two devices.

## Follow-ups (not in this PR)

- WHEP-side bitrate / encoder tuning. Out of scope here.
- GStreamer pipeline tuning (queue sizes, sync flags). Out of scope here.
