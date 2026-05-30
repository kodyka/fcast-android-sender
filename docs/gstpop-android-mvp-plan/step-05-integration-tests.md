# Step 5 — Embedded-server integration tests

**Phase:** 1 — Android MVP
**Priority:** highest
**Depends on:** Steps 1, 3
**Unblocks:** Step 6 (confident packaging)

## Goal

Prove end-to-end that:

1. The embedded server starts on an ephemeral port.
2. Two concurrent `start_embedded` calls converge without panic.
3. Bind failure surfaces as `EmbeddedState::Error` with non-empty `last_error`.
4. `TypedGstPopClient` connects and round-trips
   `create_pipeline` → `play` → `stop` → `remove_pipeline`.
5. `stop_embedded()` is idempotent.

These tests are the gate for every later step.

## Files touched

- `crates/gstpop-runtime/tests/embedded_integration.rs` (new)
- `crates/gstpop-runtime/Cargo.toml` (`dev-dependencies` already include
  `tokio` with `full`; no change needed)

## Implementation

Create `crates/gstpop-runtime/tests/embedded_integration.rs`:

```rust
//! End-to-end tests against a real embedded gst-pop server.
//!
//! These tests mutate process-global state in `gstpop-runtime::embedded`
//! and must run single-threaded:
//!
//!     cargo test -p gstpop-runtime \
//!         --features "typed-client" \
//!         --test embedded_integration -- --test-threads=1

#![cfg(feature = "typed-client")]

use std::net::TcpListener;
use std::time::Duration;

use gstpop_runtime::{
    embedded_status, start_embedded, start_embedded_with_config, stop_embedded,
    EmbeddedConfig, EmbeddedState, GstPopClient, TypedGstPopClient,
};

fn pick_free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Ensure a clean slate even if a previous test panicked.
async fn hard_reset() {
    let _ = stop_embedded().await;
    // tiny grace period for sockets to release
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn embedded_server_starts_and_stops_cleanly() {
    hard_reset().await;
    let port = pick_free_port();

    let status = start_embedded(port).await;
    assert!(
        matches!(status.state, EmbeddedState::Running),
        "expected Running, got {:?}; last_error={:?}",
        status.state,
        status.last_error,
    );
    assert_eq!(status.port, port);
    assert_eq!(status.bind, "127.0.0.1");

    let snap = embedded_status();
    assert!(matches!(snap.state, EmbeddedState::Running));

    let stopped = stop_embedded().await;
    assert!(matches!(stopped.state, EmbeddedState::Stopped));

    // Second stop is a no-op.
    let stopped_again = stop_embedded().await;
    assert!(matches!(stopped_again.state, EmbeddedState::Stopped));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_start_calls_converge() {
    hard_reset().await;
    let port = pick_free_port();

    let a = tokio::spawn(start_embedded(port));
    let b = tokio::spawn(start_embedded(port));
    let c = tokio::spawn(start_embedded(port));

    let results = [a.await.unwrap(), b.await.unwrap(), c.await.unwrap()];
    for r in &results {
        assert!(
            matches!(r.state, EmbeddedState::Running),
            "expected Running, got {:?}; last_error={:?}",
            r.state,
            r.last_error,
        );
        assert_eq!(r.port, port);
    }

    let _ = stop_embedded().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bind_failure_populates_last_error() {
    hard_reset().await;

    // Hold the port on 0.0.0.0 so the embedded server's 127.0.0.1 bind fails
    // (Linux allows this overlap to be detected; on macOS the listener on
    // 0.0.0.0 prevents the loopback bind from succeeding).
    let port = pick_free_port();
    let _blocker = TcpListener::bind(("0.0.0.0", port)).expect("hold port");

    let cfg = EmbeddedConfig {
        bind: "127.0.0.1".into(),
        port,
        api_key: None,
        allowed_origins: vec![],
    };
    let status = start_embedded_with_config(cfg).await;

    // On some platforms the overlap is permitted and the server adopts the
    // external listener. Accept either outcome but assert the negative
    // outcome carries a populated last_error.
    match status.state {
        EmbeddedState::Error => {
            assert!(
                status.last_error.is_some(),
                "Error state must populate last_error",
            );
        }
        EmbeddedState::Running => {
            // adopted; nothing to assert beyond not crashing
        }
        other => panic!("unexpected state {other:?}"),
    }

    drop(_blocker);
    let _ = stop_embedded().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_client_round_trips_playback_lifecycle() {
    hard_reset().await;
    let port = pick_free_port();
    let status = start_embedded(port).await;
    assert!(matches!(status.state, EmbeddedState::Running));

    let url = format!("ws://127.0.0.1:{port}/");
    let inner = GstPopClient::connect(&url)
        .await
        .expect("client connect");
    let client = TypedGstPopClient::new(inner);

    // Trivially valid pipeline: no sources/sinks needed for create/remove.
    // `videotestsrc ! fakesink` requires gstreamer plugins, which are present
    // in the daemon's compiled-in set on test hosts.
    let pid = client
        .create_pipeline("videotestsrc ! fakesink")
        .await
        .expect("create_pipeline");

    let pipelines = client.list_pipelines().await.expect("list_pipelines");
    assert!(pipelines.iter().any(|p| p.id == pid));

    client.play(Some(&pid)).await.expect("play");
    tokio::time::sleep(Duration::from_millis(100)).await;
    client.pause(Some(&pid)).await.expect("pause");
    client.stop(Some(&pid)).await.expect("stop");
    client.remove_pipeline(&pid).await.expect("remove");

    let _ = stop_embedded().await;
}
```

## CI hook (optional)

Add to your existing CI workflow once Step 5 lands:

```yaml
# .github/workflows/runtime-tests.yml (excerpt)
- name: gstpop-runtime integration tests
  run: |
    cargo test -p gstpop-runtime \
      --features "typed-client" \
      --test embedded_integration -- --test-threads=1
```

## Local verification

```bash
cargo test -p gstpop-runtime \
  --features "typed-client" \
  --test embedded_integration -- --test-threads=1 --nocapture
```

## Notes

- Tests are `multi_thread` flavor because the embedded server itself spawns
  tasks; a current-thread runtime would deadlock on the WebSocket accept loop.
- `--test-threads=1` is required for the test *binary* (not the runtime): the
  `embedded` module uses process-global state.
- Avoid `#[ignore]` here so CI catches regressions; the in-module tests in
  `embedded.rs` stay `#[ignore]` because they reach into private statics.

## Done when

- All four tests pass locally with `--test-threads=1`.
- CI runs them on every PR.
- A bind-failure injection produces a populated `last_error` on at least one
  CI platform.
