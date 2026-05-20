# 6 · Tighten `probe()` to a pure connectivity check

After step 5, the service owns daemon lifetime. `GstPopBackend::probe`
should no longer try to start anything — its only job is to validate
connectivity and report version + pipeline count.

## 6.1 Diff for `src/backend/gstpop/backend.rs`

Before — current behaviour (`src/backend/gstpop/backend.rs:62-66`):

```rust
async fn probe(&self) -> Result<BackendStatus> {
    if super::embedded::is_localhost(&self.url) {
        super::embedded::ensure_started(super::embedded::url_port(&self.url))
            .await
            .context("start embedded gst-pop")?;
    }

    let info = self
        .raw_call("get_version", json!({}))
        .await
        .context("probe: get_version")?;
    // …
}
```

After:

```rust
async fn probe(&self) -> Result<BackendStatus> {
    // Probe is connectivity-only. Daemon lifetime is owned by
    // GstPopService (Android) or by the user (CI / dev machine).
    let info = self
        .raw_call("get_version", json!({}))
        .await
        .context("probe: get_version (is the gst-pop service running?)")?;

    let version = info
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>");
    let count_value = self
        .raw_call("get_pipeline_count", json!({}))
        .await
        .context("probe: get_pipeline_count")?;
    let count = count_value
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Ok(BackendStatus {
        status_text: format!("gst-pop {version} - {count} pipeline(s)"),
        error_text: String::new(),
    })
}
```

## 6.2 `embedded::ensure_started` becomes orphaned

After this edit, the only in-tree caller of `ensure_started` is gone.
The shim from step 2.7 can be deleted — leave the new
`start_embedded` / `stop_embedded` / `embedded_status` API as the
public surface.

Search for stragglers:

```bash
$ rg -n 'ensure_started' src/ vendor/
src/backend/gstpop/embedded.rs:21:pub async fn ensure_started(port: u16) -> Result<()> {
```

If that's the only hit, the function and its tests can go.

## 6.3 Why this matters

- **One owner.** Today `probe()` is implicitly an "ensure" call.
  Anything that calls probe (Apply, autostart, the Refresh button)
  has a side-effect of starting a daemon. That makes the lifecycle
  hard to reason about and made the CI smoke-test port collision
  in PR #8 possible in the first place.
- **Service path is the only path.** UI → bridge → service →
  `nativeStart`. No back doors.
- **Probe failures become diagnosable.** "probe: get_version (is the
  gst-pop service running?)" tells the user something actionable;
  "start embedded gst-pop: failed to bind on 127.0.0.1:9000" required
  reading three layers of context to interpret.

## 6.4 What stays in `embedded.rs`

The full step-2 API stays. `is_localhost` and `url_port` stay (they
are used by `service.rs` from step 5). The `start_server` /
`wait_for_port` / `probe_port_open` helpers stay.

What's deleted:

- `pub async fn ensure_started(port: u16) -> Result<()>`
- The unit test that exercises it (if any).

What might be deleted later (after monitoring shows it's safe):

- The `parse_config_port` JNI helper from `src/lib.rs` (step 3.1) if
  the service ends up always passing an explicit port instead of the
  full config JSON.

## 6.5 Backwards compatibility for the CI smoke test

The smoke workflow in `.github/workflows/gstpop-smoke.yml` already
starts the dockerized gst-pop on 127.0.0.1:9000 *before* running the
Rust test binary. With the new probe, the test binary just connects
— no implicit start, no port-collision risk. PR #8's
`probe_port_open` fast path in `start_embedded` keeps the docker
listener as `externally_owned: true` for any code that still does
ask the service to start (the service itself, never the probe).

CI does **not** call the Java service code, so no test changes are
required for this step. Verify with:

```bash
$ cargo test backend::gstpop -- --include-ignored
```

Expectation: every test that previously passed without an external
listener now fails with a clear "is the gst-pop service running?"
error, and the docker-backed smoke test still passes.

Next: [07-slint-ui-state.md](./07-slint-ui-state.md).
