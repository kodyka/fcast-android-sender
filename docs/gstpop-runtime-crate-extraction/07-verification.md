# 07 — Verification, smoke checklist, rollback

The extraction is mechanical, but the daemon-lifecycle code is
load-bearing. Verify each milestone in isolation; don't batch the
manual flow.

## 7.1 Build matrix per milestone

| After step | Command | Must pass |
|---|---|---|
| 1 | `cargo build` | host build |
| 1 | `cargo build --target aarch64-linux-android` | device build |
| 1 | `cargo build -p gstpop-runtime` | empty crate compiles |
| 2 | `cargo test -p gstpop-runtime` | protocol tests, ≥1 pass |
| 2 | `cargo test --lib` | app tests still pass |
| 3 | `cargo test -p gstpop-runtime -- --ignored --test-threads=1` | embedded daemon tests |
| 4 | `cargo test --lib backend::gstpop_backend` | translate + mock-server tests |
| 5 | `cargo build --target aarch64-linux-android` | service dispatch wiring |
| 6 | `rg 'crate::backend::gstpop\|super::gstpop\|backend::gstpop' src/` | empty result |
| 6 | `rg '^use gstpop::' src/` | empty result |
| 6 | `cargo build --target aarch64-linux-android` | full build, no `gstpop` direct dep |

Any failure → don't ship that milestone; debug or revert.

## 7.2 Symbol-export sanity check

After every milestone that touches `src/lib.rs` (steps 1, 6):

```bash
nm --defined-only \
  target/aarch64-linux-android/debug/libfcastsender.so \
  | grep GstPopServiceBridge
```

Expect exactly three lines:

```
T Java_org_fcast_android_sender_GstPopServiceBridge_nativeGetGstPopServiceStatus
T Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost
T Java_org_fcast_android_sender_GstPopServiceBridge_nativeStopGstPopServiceHost
```

If a symbol disappears, the JNI side will throw `UnsatisfiedLinkError`
at runtime — there is no graceful fallback. The most likely cause is
accidentally adding `#[cfg(feature = "…")]` or wrapping the export in
the wrong `cfg` gate.

## 7.3 On-device smoke after step 6

In order, on a real device or emulator:

1. **Cold launch**, config = gst-pop, localhost
   - Pill: `Disconnected → Starting → Probing → Ready`
   - Notification: "FCast gst-pop backend — gst-pop running on 127.0.0.1:9000"
   - logcat: `Embedded gst-pop running on 127.0.0.1:9000` × 1.
2. **Background the app** (Home button)
   - Notification persists, daemon stays up.
3. **Switch to Migration**
   - Notification disappears.
   - logcat: `stop_embedded: dropping handle, previous_state=Running` × 1.
   - `adb shell "nc -z 127.0.0.1 9000 || echo DOWN"` → `DOWN`.
4. **Switch back to gst-pop**
   - Same as step 1.
5. **External daemon test**:
   ```bash
   adb shell "nc -l -p 9000 &"
   ```
   Switch to gst-pop in the UI.
   - logcat: `External gst-pop already on 127.0.0.1:9000; adopting`
   - Notification: "Using external gst-pop" → self-stop after 500 ms.
   - `adb shell pkill nc`.
6. **Process kill recovery**:
   ```bash
   adb shell am kill org.fcast.sender
   ```
   - logcat: service revived with null intent → self-stops.
   - Relaunch app → `autostart` fires → daemon restarts.

Compare each step against
[`gstpop-service-architecture.md §4-§8`](../gstpop-service-architecture.md).
The runtime behaviour must be byte-for-byte identical — the
extraction is a pure refactor.

## 7.4 Performance sanity

The move shouldn't cost anything measurable, but check:

```bash
# Cold start to "Ready" timing — compare before/after.
adb logcat -s GstPopService:D | \
  grep -E "onStartCommand action=org.fcast.android.sender.GSTPOP_START|nativeStart -> "
# Subtract timestamps. Expect <500 ms diff between Start command and
# nativeStart return.
```

Also confirm the cdylib didn't grow significantly:

```bash
ls -la target/aarch64-linux-android/release/libfcastsender.so
# Expect ±2% vs main. The cdylib contains gstpop-runtime's code either
# way — the move doesn't change the link set.
```

## 7.5 Rollback plan

Each milestone is its own PR. Rollback is `git revert <pr>`:

| If broken after step | Revert | What you keep |
|---|---|---|
| 1 | revert M1 | All prior code; lose only the empty crate. |
| 2 | revert M2 | M1's workspace skeleton; tests stay in the app. |
| 3 | revert M3 | embedded.rs back in the app; lifecycle untouched. |
| 4 | revert M4 | `GstPopBackend` back under `gstpop/`. |
| 5 | revert M5 | `service.rs` back under `gstpop/`. |
| 6 | revert M6 | Re-exports stay in place; app keeps building via the bridge module. |

Because each PR independently builds, you can revert the *last*
green PR's successor without unwinding the whole chain.

If the rollback target is "we don't want a workspace crate at all",
revert M6 → M1 in order. All other code is unchanged; you'll be back
to the pre-extraction layout.

## 7.6 Post-merge follow-ups

These are *not* in scope for the extraction but become easier
afterwards:

- **Port-switch fix** (called out in
  `gstpop-service-architecture.md §10`). With the runtime in its own
  crate, the change is contained to `embedded.rs` + its tests.
- **JNI symbol grep in CI** (called out in the guide §10.7). The
  symbol names don't change; the check is more valuable now that the
  symbols' definition module moved.
- **Robolectric tests for `GstPopServiceBridge`** (guide §10.3). Live
  on the Java side; orthogonal to this refactor.
- **`crates/jni-glue`** — extract `HOST_RUNTIME`, `android_context()`,
  `migration_service`, `gstpop_service`, and the JNI exports into a
  single binding crate. Big move; tackle only if the app crate
  becomes unwieldy.

## 7.7 Don't do these as part of the extraction

- Renaming public types (`EmbeddedStatus`, `EmbeddedState`). The
  Serde representation flows over JNI to Java; a rename is a
  protocol break.
- Adding new public functions to `gstpop-runtime`. Keep the crate's
  API surface a strict subset of what the app already used.
- Touching `vendor/gstpop`. It is its own workspace member; the
  extraction does not need any upstream changes.

That's the plan. Start at [01-workspace-skeleton.md](./01-workspace-skeleton.md).
