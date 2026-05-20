# 2 · Rust daemon control API

Today `src/backend/gstpop/embedded.rs` exposes one public function:

```rust
pub async fn ensure_started(port: u16) -> Result<()>;
```

That's "implicit start, no stop, no status". Split it into an explicit
lifecycle that the service can drive.

## 2.1 Public types

Add to the top of `src/backend/gstpop/embedded.rs`:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use serde::Serialize;

use gstpop::{
    gst::{create_event_channel, PipelineManager},
    server::{ServerConfig, ServerHandle},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddedState {
    Stopped,
    Starting,
    Running { externally_owned: bool },
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddedStatus {
    pub state: EmbeddedState,
    pub bind: String,
    pub port: u16,
    pub last_error: Option<String>,
    pub started_at_unix_ms: Option<u64>,
}

impl EmbeddedStatus {
    fn snapshot() -> Self {
        let st = STATE.read();
        Self {
            state: st.state,
            bind: st.bind.clone(),
            port: st.port,
            last_error: st.last_error.clone(),
            started_at_unix_ms: st.started_at_unix_ms,
        }
    }
}

#[derive(Default)]
struct InnerState {
    state: EmbeddedState,
    bind: String,
    port: u16,
    last_error: Option<String>,
    started_at_unix_ms: Option<u64>,
}

impl Default for EmbeddedState {
    fn default() -> Self { Self::Stopped }
}
```

The `EmbeddedStatus` derives `Serialize` so the JNI layer in step 3
can hand it straight to Java as JSON without manual stringification.

## 2.2 Statics

```rust
// Race control — only one start_embedded call wins.
static CLAIMED: AtomicBool = AtomicBool::new(false);
// Set true *only* after the WebSocket task is accepting connections.
static READY:   AtomicBool = AtomicBool::new(false);

// Observable state surface. RwLock because reads (status queries from
// the UI) are far more frequent than writes (start / stop / fail).
static STATE:   RwLock<InnerState> = parking_lot::const_rwlock(InnerState {
    state: EmbeddedState::Stopped,
    bind: String::new(),
    port: 0,
    last_error: None,
    started_at_unix_ms: None,
});

// Owned ServerHandle. `Option<None>` means either we never started, or
// the listener belongs to someone else (externally_owned: true). Wrap
// in parking_lot::Mutex (not std) so we don't poison across panics.
static HANDLE:  parking_lot::Mutex<Option<ServerHandle>> =
    parking_lot::const_mutex(None);
```

(`parking_lot::const_rwlock` and `const_mutex` exist for static
initialisers. If you prefer `Lazy<…>` that's fine too.)

## 2.3 `start_embedded`

```rust
/// Start the embedded gst-pop server (idempotent). Returns the
/// resulting status. Never panics — failures are reflected in
/// `EmbeddedStatus::state == Error` with `last_error` populated.
pub async fn start_embedded(port: u16) -> EmbeddedStatus {
    // Fast path: already fully running on this exact port.
    if READY.load(Ordering::Acquire) && STATE.read().port == port {
        return EmbeddedStatus::snapshot();
    }

    // External listener on the requested port — adopt it.
    if probe_port_open(port).await {
        let mut st = STATE.write();
        st.state = EmbeddedState::Running { externally_owned: true };
        st.bind = "127.0.0.1".into();
        st.port = port;
        st.last_error = None;
        st.started_at_unix_ms = Some(now_unix_ms());
        drop(st);
        CLAIMED.store(true, Ordering::Release);
        READY.store(true, Ordering::Release);
        tracing::info!("External gst-pop already on 127.0.0.1:{port}; adopting");
        return EmbeddedStatus::snapshot();
    }

    // Race to be the one that starts the server.
    if CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        // Lost the race — wait for the winner.
        let _ = wait_for_port(port).await;
        return EmbeddedStatus::snapshot();
    }

    // Won the race. Mark Starting before any await.
    {
        let mut st = STATE.write();
        st.state = EmbeddedState::Starting;
        st.bind = "127.0.0.1".into();
        st.port = port;
        st.last_error = None;
        st.started_at_unix_ms = None;
    }

    match start_server(port).await {
        Ok(handle) => {
            *HANDLE.lock() = Some(handle);
            READY.store(true, Ordering::Release);
            let mut st = STATE.write();
            st.state = EmbeddedState::Running { externally_owned: false };
            st.started_at_unix_ms = Some(now_unix_ms());
            tracing::info!("Embedded gst-pop running on 127.0.0.1:{port}");
        }
        Err(e) => {
            CLAIMED.store(false, Ordering::Release);
            let mut st = STATE.write();
            st.state = EmbeddedState::Error;
            st.last_error = Some(format!("{e:#}"));
            tracing::error!(?e, "Embedded gst-pop bind failed");
        }
    }
    EmbeddedStatus::snapshot()
}
```

## 2.4 `stop_embedded`

```rust
/// Stop the embedded gst-pop server if we own it. No-op if the
/// listener is externally owned or already stopped.
pub async fn stop_embedded() -> EmbeddedStatus {
    // Wait briefly if a start is in flight so we don't leave a half-bound
    // ServerHandle behind.
    for _ in 0..10 {
        if !matches!(STATE.read().state, EmbeddedState::Starting) { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let externally_owned = matches!(
        STATE.read().state,
        EmbeddedState::Running { externally_owned: true }
    );
    if externally_owned {
        tracing::info!("stop_embedded: listener is externally owned; no-op");
        return EmbeddedStatus::snapshot();
    }

    if let Some(handle) = HANDLE.lock().take() {
        // Drop synchronously so the bind is released before we return.
        drop(handle);
    }

    READY.store(false, Ordering::Release);
    CLAIMED.store(false, Ordering::Release);
    let mut st = STATE.write();
    st.state = EmbeddedState::Stopped;
    st.last_error = None;
    st.started_at_unix_ms = None;
    EmbeddedStatus::snapshot()
}
```

## 2.5 `embedded_status`

```rust
/// Cheap snapshot. Does not perform any network I/O — relies on
/// state set by start/stop. The UI can call this on a poll loop
/// without worrying about the cost.
pub fn embedded_status() -> EmbeddedStatus {
    EmbeddedStatus::snapshot()
}
```

## 2.6 Helpers (existing, kept)

```rust
async fn start_server(port: u16) -> Result<ServerHandle> {
    let (event_tx, _) = create_event_channel();
    let manager = Arc::new(PipelineManager::new(event_tx.clone()));
    let config = ServerConfig {
        bind: "127.0.0.1".to_string(),
        port,
        no_websocket: false,
        no_dbus: true,
        api_key: None,
        allowed_origins: Vec::new(),
    };
    let handle = ServerHandle::start(config, Arc::clone(&manager), &event_tx)
        .await
        .map_err(|()| anyhow!("failed to bind embedded gst-pop on 127.0.0.1:{port}"))?;
    wait_for_port(port).await?;
    Ok(handle)
}

async fn probe_port_open(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    matches!(
        tokio::time::timeout(
            Duration::from_millis(200),
            tokio::net::TcpStream::connect(&addr),
        )
        .await,
        Ok(Ok(_))
    )
}

async fn wait_for_port(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(&addr).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    anyhow::bail!("embedded gst-pop did not start on port {port} within 2s")
}

pub fn is_localhost(url: &str) -> bool {
    url.contains("127.0.0.1") || url.contains("localhost") || url.contains("[::1]")
}

pub fn url_port(url: &str) -> u16 {
    url.rsplit(':')
        .next()
        .and_then(|s| s.trim_end_matches('/').parse::<u16>().ok())
        .unwrap_or(9000)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
```

## 2.7 `ensure_started` — keep as a compatibility shim during M2

```rust
/// Compatibility shim. Identical to start_embedded for the
/// "must-be-running" case, but returns Result<()> for callers that
/// don't care about the new EmbeddedStatus. **Delete after step 6
/// removes the last in-tree caller.**
pub async fn ensure_started(port: u16) -> Result<()> {
    let status = start_embedded(port).await;
    match status.state {
        EmbeddedState::Running { .. } => Ok(()),
        EmbeddedState::Error => Err(anyhow!(status.last_error.unwrap_or_default())),
        _ => Err(anyhow!("gst-pop not running: {:?}", status.state)),
    }
}
```

## 2.8 Sanity-test against the existing port-binding tests

Add to the same file under `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn start_then_stop_is_idempotent() {
    let port = pick_free_port();
    let a = start_embedded(port).await;
    assert!(matches!(a.state, EmbeddedState::Running { externally_owned: false }));
    let b = start_embedded(port).await;
    assert!(matches!(b.state, EmbeddedState::Running { externally_owned: false }));
    let c = stop_embedded().await;
    assert!(matches!(c.state, EmbeddedState::Stopped));
}

#[tokio::test]
async fn external_listener_is_adopted_and_not_killed() {
    let port = pick_free_port();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let a = start_embedded(port).await;
    assert!(matches!(a.state, EmbeddedState::Running { externally_owned: true }));
    let b = stop_embedded().await;
    // External listener still alive.
    assert!(matches!(b.state, EmbeddedState::Running { externally_owned: true }));
    drop(listener);
}

fn pick_free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}
```

The "external listener" branch is now an explicit, testable case
rather than a fall-through condition inside `ensure_started`.

Next: [03-jni-and-java-bridge.md](./03-jni-and-java-bridge.md).
