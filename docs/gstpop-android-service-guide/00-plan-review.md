# 0 · Plan review

The 15-step plan you wrote is sound. A few deltas based on the current
code that you should bake into the work as you go:

## 1. There is no Kotlin/Java orchestration layer today

All glue lives in
`app/src/main/java/org/fcast/android/sender/MainActivity.java`
(`NativeActivity` subclass; ~1158 lines) and a single sibling service
`ScreenCaptureService.java`. Slint runs entirely inside the Rust
`cdylib` via `android-activity`. The "Java bridge class" idea is
correct; just be aware you are *creating* that layer, not extending an
existing controller.

## 2. A foreground-service template already exists

`ScreenCaptureService` (`mediaProjection` type) is the closest
analogue. The new `GstPopService` should mirror its notification +
`START_STICKY` shape, **not** invent a new pattern. See
`app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java`.

## 3. JNI naming is load-bearing

Every new native must be exported as
`Java_org_fcast_android_sender_<Class>_<method>` to match the declared
`package`. Don't free-form the symbol names.

## 4. Implicit startup is *partially* gated already

PR #8 added a pre-bind TCP probe in `src/backend/gstpop/embedded.rs`
(lines 21–62) so `ensure_started` defers to an external listener if
one is present. Your plan to "remove the implicit start from probe"
is still correct — this guide just notes the current state is a soft
defer, not a hard removal.

## 5. Slint already exposes the state surface the UI needs

The `Bridge` global has `media-backend-state` (enum
`disconnected | probing | ready | error`),
`media-backend-status-text`, and `media-backend-error-text` in
`ui/bridge.slint:289-296`. We only need to enrich the existing state
machine, not invent a new one. Adding a `Starting` variant to
`MediaBackendState` is recommended — see
[07-slint-ui-state.md](./07-slint-ui-state.md).

## 6. `BackendLifecycle` is the single funnel

For backend changes from Slint. The Apply/Save/Probe wiring is in
`src/backend/lifecycle.rs:31-85`. That's where the service-start hook
plugs in — see [05-rewire-lifecycle.md](./05-rewire-lifecycle.md).

## 7. `migration` backend is unrelated to this work

Moving gst-pop into a service does not change `MigrationBackend`'s
in-process runtime, and the *runtime command* path is still Java's
`nativeGraphCommand` → Rust → `migration::runtime` regardless of the
selected media backend. Your plan's "potential concerns" note is
accurate and should stay out of scope here.

## Resulting order of operations

```
M1. Decide continuity contract                   (your steps 1–2)
M2. Refactor Rust daemon control into            (steps 3, 8)
    start/stop/status APIs
M3. Add JNI entrypoints + Java bridge            (steps 4–5)
M4. Add GstPopService                            (steps 6, 7, 10, 11)
M5. Rewire BackendLifecycle::apply               (steps 8–9)
M6. Tighten probe() to a pure connectivity check (step 15)
M7. Slint UI state additions + propagation       (steps 9, 13)
M8. Tests + cleanup                              (steps 14, 15)
```

Next: [01-continuity-contract.md](./01-continuity-contract.md).
