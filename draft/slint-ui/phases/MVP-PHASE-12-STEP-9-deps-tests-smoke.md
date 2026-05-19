# MVP-PHASE-12 — Step 9: Cargo deps, test suite, end-to-end smoke

> Part 9 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Previous: [STEP-8](./MVP-PHASE-12-STEP-8-lifecycle-and-status-writeback.md).

---

## 0. Goal of this step

Close the phase: add the new Cargo deps, expand the test suite,
script a reproducible smoke run against
`ghcr.io/dabrain34/gstpop:latest`, and capture the operator notes a
follow-on phase will want.

---

## 1. Cargo deps

Append to `Cargo.toml` (under the existing `[dependencies]` table,
preserving ordering by feature area):

```toml
# MVP-PHASE-12 — gst-pop WebSocket adapter
tokio-tungstenite = "0.26"
futures-util      = { version = "0.3", default-features = false, features = ["sink", "std"] }
async-trait       = "0.1"
once_cell         = "1"
```

> **Why pin `tokio-tungstenite = "0.26"`?** That's the version
> gst-popctl itself uses
> ([`gstpop/client/rust/Cargo.toml`](https://github.com/dabrain34/gstpop/blob/main/client/rust/Cargo.toml)).
> Staying in lockstep means the wire protocol our adapter speaks is
> verified against the upstream client in CI.
>
> **Why `default-features = false` on `futures-util`?** The full
> default-feature set drags in `futures-macro` and a small
> proc-macro graph that adds ~3 s to a cold compile. We only need
> `Sink::send` and `Stream::next`, which the `"sink"` and `"std"`
> features cover.
>
> **Why `once_cell` even though `Lazy` will become stable?** The
> repo's MSRV is whatever ships with the Android NDK's bundled
> rustc; safer to keep the explicit dep.

Run:

```sh
cargo update -p tokio-tungstenite
cargo build  --target aarch64-linux-android
cargo build  --target armv7-linux-androideabi
cargo build  --target x86_64-linux-android
cargo build  --target i686-linux-android
```

All four targets must succeed (the
`package.metadata.android.build_targets` list at
`Cargo.toml:48` is the source of truth).

---

## 2. Test suite

### 2.1 Protocol unit tests (added in STEP-6)

Already covered in `src/backend/gstpop/protocol_tests.rs` — three
classifier tests. Expand with:

```rust
#[test]
fn deserializes_a_full_pipeline_added_event() {
    let text = r#"{
        "event":"pipeline_added",
        "data":{"pipeline_id":"7","description":"videotestsrc ! autovideosink"}
    }"#;
    match classify(text) {
        ClassifiedFrame::Event(Event::PipelineAdded { pipeline_id, description }) => {
            assert_eq!(pipeline_id, "7");
            assert_eq!(description, "videotestsrc ! autovideosink");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn response_with_error_does_not_yield_result() {
    let text = r#"{
        "id":"abc",
        "error":{"code":-32000,"message":"Pipeline not found"}
    }"#;
    match classify(text) {
        ClassifiedFrame::Response(rsp) => {
            assert!(rsp.result.is_none());
            assert_eq!(rsp.error.as_ref().unwrap().code, -32000);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn integer_id_responses_round_trip_to_string() {
    let text = r#"{"id":42,"result":{}}"#;
    match classify(text) {
        ClassifiedFrame::Response(rsp) => {
            assert_eq!(rsp.id_as_str(), Some("42".to_owned()));
        }
        other => panic!("unexpected: {other:?}"),
    }
}
```

### 2.2 Migration adapter tests (STEP-5)

Already two smoke tests at the bottom of
`src/backend/migration_backend.rs`. Add:

```rust
#[tokio::test]
async fn shutdown_is_idempotent() {
    let backend = MigrationBackend::new();
    backend.shutdown().await.expect("first shutdown ok");
    backend.shutdown().await.expect("second shutdown ok");
}

#[tokio::test]
async fn dispatch_surfaces_errors_from_runtime() {
    let backend = MigrationBackend::new();
    let result = backend
        .dispatch("nonsense", json!({ "id": "nope" }))
        .await;
    assert!(result.is_err(), "{result:?}");
}
```

### 2.3 GstPop adapter tests (STEP-7)

Three translate-table tests (already in §6 of STEP-7) + the
`probe_against_docker` integration test gated behind `#[ignore]`.

Add a roundtrip test against a fake echo server (no Docker required):

```rust
#[tokio::test]
async fn round_trip_against_echo_server() {
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    use futures_util::{SinkExt, StreamExt};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Echo server replies to every request with a fake gst-pop
    // get_version-shaped response.
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            while let Some(msg) = ws.next().await {
                if let Ok(tokio_tungstenite::tungstenite::Message::Text(text)) = msg {
                    let req: serde_json::Value =
                        serde_json::from_str(text.as_str()).unwrap();
                    let id = req["id"].as_str().unwrap().to_owned();
                    let reply = serde_json::json!({
                        "id": id,
                        "result": { "version": "test-0.0", "count": 0 }
                    });
                    let _ = ws.send(tokio_tungstenite::tungstenite::Message::Text(
                        reply.to_string().into(),
                    )).await;
                }
            }
        }
    });

    let backend = GstPopBackend::new(
        format!("ws://127.0.0.1:{port}"),
        None,
        "0".into(),
    );
    let value = backend
        .raw_call("get_version", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(value["version"], "test-0.0");
}
```

> **Why an echo server, not a real daemon?** The test runs in CI
> where GStreamer and meson aren't installed. Echoing
> JSON-RPC-shaped responses is enough to exercise the
> id-correlation, connect, and read-loop logic.

### 2.4 Lifecycle tests

`src/backend/lifecycle.rs` doesn't compile without a `MainWindow`
(which requires a Slint event loop), so its tests run only on a
real Android device. STEP-8 §5 documents the manual smoke; CI tests
the helpers in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_round_trip_through_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        let original = StoredBackendConfig::defaults();
        original.save(&path).unwrap();
        let restored = StoredBackendConfig::load(&path).unwrap();

        assert_eq!(restored.kind, BackendKind::Migration);
        assert_eq!(restored.gstpop_url, "ws://127.0.0.1:9000");
    }

    #[test]
    fn load_falls_back_to_defaults_when_file_missing() {
        let dir = tempdir().unwrap();
        let loaded = StoredBackendConfig::load(&dir.path().to_path_buf()).unwrap();
        assert_eq!(loaded.kind, BackendKind::Migration);
    }
}
```

(Add `tempfile = "3"` to `[dev-dependencies]` if not present.)

---

## 3. End-to-end smoke (against Docker)

`scripts/smoke-gstpop.sh` (new):

```sh
#!/usr/bin/env bash
# MVP-PHASE-12 — end-to-end smoke test against ghcr.io/dabrain34/gstpop:latest.
#
# Boots a gst-pop daemon in Docker, builds & runs the in-tree
# integration test against it, then tears down. Requires:
#   - docker
#   - cargo + the aarch64-linux-android toolchain (for the cross build)
#   - host running on amd64 or arm64 (the image is multi-arch)
#
# Usage:  bash scripts/smoke-gstpop.sh

set -euo pipefail

DAEMON_ID=$(docker run --rm -d \
    -p 127.0.0.1:9000:9000 \
    --name gst-pop-smoke-$$ \
    ghcr.io/dabrain34/gstpop:latest)

cleanup() {
    docker stop "$DAEMON_ID" >/dev/null || true
}
trap cleanup EXIT

# Wait for the daemon to bind.
for i in {1..30}; do
    if curl -s --max-time 1 http://127.0.0.1:9000 >/dev/null 2>&1; then break; fi
    sleep 0.2
done

# Build against the host triple so we don't need the Android SDK.
cargo test --no-default-features \
    --target "$(rustc -vV | awk '/host/ {print $2}')" \
    -p android-sender backend::gstpop -- --include-ignored

# Optional: also pin the cross-built target compiles cleanly.
cargo build --target aarch64-linux-android
```

Make it executable + add to PHASE-12 docs (do *not* commit a CI
job — the Docker pull is too heavy for the standard CI matrix).

### 3.1 Expected smoke output

```
$ bash scripts/smoke-gstpop.sh
Unable to find image 'ghcr.io/dabrain34/gstpop:latest' locally
latest: Pulling from dabrain34/gstpop
…
Status: Downloaded newer image for ghcr.io/dabrain34/gstpop:latest
   Compiling android-sender v0.1.0 (/home/ubuntu/repos/fcast-android-sender)
    Finished test [unoptimized + debuginfo] target(s) in 4.31s
     Running unittests src/lib.rs
running 3 tests
test backend::gstpop::backend::tests::translate_passes_through_native_verbs ... ok
test backend::gstpop::backend::tests::translate_maps_start_to_play ... ok
test backend::gstpop::backend::tests::probe_against_docker ... ok
test result: ok. 3 passed; 0 failed
```

---

## 4. Operator notes

For each test box / CI lane that runs the smoke:

| Concern | Detail |
|---|---|
| **Image pull size** | ~120 MB. CI lanes that don't already pull the gst-pop image should run the smoke nightly, not per-PR. |
| **Port conflicts** | The script binds `127.0.0.1:9000`. If something else on the box uses that port, the test fails with `connect_async` errors. |
| **API key** | The script does not pass `--api-key`, so the daemon is open. For prod test fixtures, set `GSTPOP_API_KEY=…` and update the script to pass it through. |
| **Multi-arch** | The image is amd64 + arm64. Aarch64 dev boxes work; armv7 emulation is not supported by upstream. |

---

## 5. PR checklist for the implementer

When the phase ships, the PR description should include:

- [ ] All 9 step files merged (the docs already in this directory).
- [ ] STEP-1 grep ladder rerun, anchors still valid.
- [ ] `cargo build` green on all 4 Android targets.
- [ ] `cargo test backend::` green on host triple.
- [ ] `bash scripts/smoke-gstpop.sh` green when Docker is
      available.
- [ ] Manual smoke (STEP-8 §5) recorded with `scrcpy` and linked
      from the PR.
- [ ] `Cargo.lock` updated and reviewed.

---

## 6. Follow-on phases

| Phase | What | Why |
|---|---|---|
| **PHASE-13** | Composer — translate node-graph mutations into gst-launch strings so existing migration call sites work against gst-pop | The largest remaining gap; would let PHASE-11's mixer screen run unchanged against either backend. |
| **PHASE-14** | Auto-reconnect + WiFi-handoff lifecycle for `GstPopClient` | Today the client drops cached connections on failure and reconnects on next call. Auto-keepalive + jittered reconnect makes the UX feel native. |
| **PHASE-15** | Per-pipeline UI — `Bridge.gstpop-pipeline-id` becomes a picker (driven by `list_pipelines`) instead of a free-form `LineEdit` | Lets the user attach the app to an already-running pipeline on a shared daemon. |
| **PHASE-16** | Subscribe to `pipeline_added` / `pipeline_removed` / `eos` events and surface them in the existing debug log (`Bridge.log-entries`) | Brings broadcast events into the existing PHASE-9 debug surface so testers see lifecycle without `tail -f`. |

---

## 7. Exit gate

- [ ] All four Cargo deps added; `cargo update -p tokio-tungstenite`
      completes; `cargo build` green on every Android target.
- [ ] §2's test suite passes on the host triple.
- [ ] `scripts/smoke-gstpop.sh` passes when Docker is available
      (manual run; not yet a CI lane).
- [ ] Operator notes (§4) reviewed for the org's specific CI box.

This closes PHASE-12. Move to PHASE-13 when ready to retarget
migration call sites onto the new trait.
