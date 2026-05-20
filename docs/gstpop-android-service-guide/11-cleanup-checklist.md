# 11 · Cleanup checklist

After the service path is working end-to-end (step 10's manual flow
all green), do these in order. Each item is independently revertible.

## 11.1 Delete the compatibility shim

`src/backend/gstpop/embedded.rs::ensure_started` (step 2.7) has no
callers after step 6. Delete the function and its tests.

Search and confirm:

```bash
$ rg -n 'ensure_started' src/ vendor/ app/ ui/
# Expect: no matches.
```

## 11.2 Optional: collapse atomics into `STATE`

`CLAIMED` and `READY` are race controls layered over `EmbeddedState`.
The lock-free fast path in `start_embedded` is what justifies them.
If profiling shows the `RwLock::read` on the fast path is too
expensive (it isn't — `parking_lot::RwLock` reads are ~10ns
uncontended), you can collapse to:

```rust
static STATE: RwLock<InnerState> = parking_lot::const_rwlock(/* … */);

pub async fn start_embedded(port: u16) -> EmbeddedStatus {
    if matches!(STATE.read().state, EmbeddedState::Running { .. }) {
        return EmbeddedStatus::snapshot();
    }
    // … rest of the function, gating with STATE.write() transitions …
}
```

Otherwise leave the atomics in place — they document the
"who-starts-the-server" invariant explicitly.

## 11.3 Drop public `is_localhost` if not used outside the module

After step 5, `embedded::is_localhost` is called from `service.rs`.
That's the same module subtree, so the visibility can drop from `pub`
to `pub(super)`. The `url_port` helper has the same shape.

```rust
// src/backend/gstpop/embedded.rs
pub(super) fn is_localhost(url: &str) -> bool { /* … */ }
pub(super) fn url_port(url: &str) -> u16 { /* … */ }
```

Defensive: leave them `pub` if you suspect a future caller. The
visibility tightening is a paper cut, not a correctness fix.

## 11.4 Add `MediaBackend::shutdown` to the apply switch path

Already in step 5.5. If you skipped it:

```rust
if previous.kind() != config.kind {
    if let Err(err) = previous.shutdown().await {
        tracing::warn!(?err, "previous backend shutdown failed");
    }
}
```

Without this the WebSocket client connection from `GstPopBackend`
lingers after a switch to Migration. Not a correctness bug — just a
resource leak that compounds across many switches.

## 11.5 Remove the duplicate `vm_as_ptr` / `activity_as_ptr` blocks

Once `android_context()` from step 5.2 is in place, the inline
`vm_as_ptr` / `activity_as_ptr` blocks at `src/lib.rs:591-610` and
`src/lib.rs:1146-1156` can be replaced with `android_context()?`.

This is a refactor; it doesn't change behaviour but it reduces the
blast radius if you ever need to change how the VM handle is
acquired.

## 11.6 Tighten the foreground notification text

Step 4.1's `describe()` returns generic strings. After integration,
swap them for `@tr`-ready resource strings so they can be localised:

```java
// app/src/main/res/values/strings.xml
<string name="gstpop_running">gst-pop running on %1$s:%2$d</string>
<string name="gstpop_starting">Starting gst-pop on %1$s:%2$d…</string>
<string name="gstpop_stopped">gst-pop stopped</string>
<string name="gstpop_error">gst-pop error: %1$s</string>
```

```java
case "running":
    return getString(R.string.gstpop_running, bind, port);
```

Defer until you actually have non-English locales — premature
localisation is just more strings to maintain.

## 11.7 Surface the externally-owned hint in the notification

Currently the notification text says "gst-pop running on
127.0.0.1:9000" even when the listener belongs to a manually-started
daemon. Append a hint:

```java
boolean external = o.optBoolean("externally_owned", false);
if (external) {
    return "Using external gst-pop on " + bind + ":" + port;
}
```

Same fix applied in the UI status pill (step 7.4 already shows it).

## 11.8 Doc the lifecycle in `README.md`

Add a section to the repo root README pointing at this guide:

```markdown
### Media backend

`migration` is the default in-process media runtime. `gst-pop` runs
the upstream gst-pop daemon, hosted by the in-process
`GstPopService` foreground service when running on Android. See
[docs/gstpop-android-service-guide/](docs/gstpop-android-service-guide/)
for the lifecycle contract.
```

## 11.9 Wire CI to grep for the new JNI symbols

See step 10.7. The `nm | grep -c GstPopServiceBridge` check catches
a class of regressions where someone refactors the JNI symbol names
and forgets to re-export them — the cdylib still builds, but Java
fails at runtime with `UnsatisfiedLinkError`.

## 11.10 What we are deliberately *not* cleaning up

- `vendor/gstpop/src/dbus/*` — unrelated to the Android service.
  Leave the upstream-vendored DBus code in place; it's gated by
  `cfg(target_os = "linux")` and doesn't reach Android.
- `ScreenCaptureService.java` — completely independent. The two
  services coexist (different `foregroundServiceType`, different
  notification channel, different responsibilities).
- The existing migration-runtime path through `nativeGraphCommand` —
  also unrelated. The runtime layer doesn't know which media
  backend is selected.

Next: [12-open-decisions.md](./12-open-decisions.md).
