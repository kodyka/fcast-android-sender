# Step 13 — Signal handling (CLI only)

**Phase:** 3 — Desktop & cross-platform
**Priority:** low
**Depends on:** Step 12
**Unblocks:** clean Ctrl-C / SIGTERM behavior for the desktop CLI

## Goal

Port upstream `daemon/src/signal.rs` into the new `gstpop-cli` crate **only**.
The Android runtime must remain free of process-signal lifecycle: Android
controls its own service/activity lifecycle, and a tokio signal handler
inside an app process would interfere with platform behavior.

## Files touched

- `crates/gstpop-cli/src/signal.rs` (new)
- `crates/gstpop-cli/Cargo.toml` (`tokio` with `signal` feature)
- (Already referenced by `crates/gstpop-cli/src/cmd/daemon.rs` in Step 12)

## Implementation

### 1. Manifest

`tokio`'s `full` feature already includes `signal`. If you trimmed features,
ensure `signal` is enabled in `crates/gstpop-cli/Cargo.toml`:

```toml
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "signal"] }
```

### 2. The signal module

Create `crates/gstpop-cli/src/signal.rs`:

```rust
//! Cross-platform shutdown signal handling for the desktop CLI.
//!
//! - On Unix: waits for SIGINT or SIGTERM.
//! - Elsewhere: waits for Ctrl-C.
//!
//! Returns once a signal arrives so the caller can perform a graceful
//! shutdown. Never call this from the Android runtime — Android lifecycle
//! is owned by the OS, and intercepting SIGTERM in an app process is a
//! foot-gun.

#[cfg(unix)]
pub async fn wait_for_shutdown() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt())
        .expect("install SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate())
        .expect("install SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {
            tracing::info!("received SIGINT, shutting down");
        }
        _ = sigterm.recv() => {
            tracing::info!("received SIGTERM, shutting down");
        }
    }
}

#[cfg(not(unix))]
pub async fn wait_for_shutdown() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::error!(?e, "failed to listen for Ctrl-C");
    } else {
        tracing::info!("received Ctrl-C, shutting down");
    }
}
```

### 3. Use from `daemon` subcommand

Already wired in [Step 12](./step-12-desktop-cli-crate.md):

```rust
crate::signal::wait_for_shutdown().await;
gstpop_runtime::stop_embedded().await;
```

## Test (Unix)

`crates/gstpop-cli/tests/signal.rs`:

```rust
//! Verifies the daemon subcommand exits cleanly on SIGTERM.
//! Skipped on non-Unix targets.

#![cfg(unix)]

use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn daemon_exits_on_sigterm() {
    let bin = env!("CARGO_BIN_EXE_gstpop");

    let mut child = Command::new(bin)
        .args(["daemon", "--port", "9123"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon");

    std::thread::sleep(Duration::from_millis(500));

    // SIGTERM
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }

    // Should exit within ~2 seconds.
    for _ in 0..40 {
        if let Some(status) = child.try_wait().expect("try_wait") {
            assert!(status.success() || status.code() == Some(0));
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    panic!("daemon did not exit on SIGTERM within 2s");
}
```

Add `libc` to `crates/gstpop-cli/Cargo.toml` under `[dev-dependencies]` if
not already present:

```toml
[dev-dependencies]
libc = "0.2"
```

## Anti-goals (do NOT do these)

- ❌ Add `tokio::signal::unix` anywhere in `gstpop-runtime`.
- ❌ Install SIGTERM handlers from the JNI bridge.
- ❌ Expose `wait_for_shutdown` from the workspace public API; it's
  CLI-internal.
- ❌ Use `ctrl_c()` on Unix — the Unix-specific handler is needed for
  SIGTERM (which `ctrl_c()` does not cover) for systemd / Docker / k8s
  shutdown paths.

## Done when

- `gstpop daemon --port N` runs until SIGINT or SIGTERM, then stops the
  embedded server cleanly.
- The `daemon_exits_on_sigterm` test passes on Linux and macOS.
- `gstpop-runtime` contains zero references to `tokio::signal`.
