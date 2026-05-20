use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
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
    Running,
    Error,
}

impl Default for EmbeddedState {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddedStatus {
    pub state: EmbeddedState,
    pub externally_owned: bool,
    pub bind: String,
    pub port: u16,
    pub last_error: Option<String>,
    pub started_at_unix_ms: Option<u64>,
}

#[derive(Default)]
struct InnerState {
    state: EmbeddedState,
    externally_owned: bool,
    bind: String,
    port: u16,
    last_error: Option<String>,
    started_at_unix_ms: Option<u64>,
}

// Race control — only one start_embedded call wins.
static CLAIMED: AtomicBool = AtomicBool::new(false);
// Set true only after the WebSocket task is accepting connections.
static READY: AtomicBool = AtomicBool::new(false);

static STATE: Lazy<parking_lot::RwLock<InnerState>> =
    Lazy::new(|| parking_lot::RwLock::new(InnerState::default()));

static HANDLE: Lazy<parking_lot::Mutex<Option<ServerHandle>>> =
    Lazy::new(|| parking_lot::Mutex::new(None));

fn snapshot() -> EmbeddedStatus {
    let st = STATE.read();
    EmbeddedStatus {
        state: st.state,
        externally_owned: st.externally_owned,
        bind: st.bind.clone(),
        port: st.port,
        last_error: st.last_error.clone(),
        started_at_unix_ms: st.started_at_unix_ms,
    }
}

/// Start the embedded gst-pop server (idempotent). Returns the resulting
/// status. Never panics — failures are reflected in state == Error.
pub async fn start_embedded(port: u16) -> EmbeddedStatus {
    // Fast path: already fully running on this exact port.
    if READY.load(Ordering::Acquire) && STATE.read().port == port {
        return snapshot();
    }

    // External listener on the requested port — adopt it.
    if probe_port_open(port).await {
        let mut st = STATE.write();
        st.state = EmbeddedState::Running;
        st.externally_owned = true;
        st.bind = "127.0.0.1".into();
        st.port = port;
        st.last_error = None;
        st.started_at_unix_ms = Some(now_unix_ms());
        drop(st);
        CLAIMED.store(true, Ordering::Release);
        READY.store(true, Ordering::Release);
        tracing::info!("External gst-pop already on 127.0.0.1:{port}; adopting");
        return snapshot();
    }

    // Race to be the one that starts the server.
    if CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        // Lost the race — wait for the winner.
        let _ = wait_for_port(port).await;
        return snapshot();
    }

    // Won the race. Mark Starting before any await.
    {
        let mut st = STATE.write();
        st.state = EmbeddedState::Starting;
        st.externally_owned = false;
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
            st.state = EmbeddedState::Running;
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
    snapshot()
}

/// Stop the embedded gst-pop server if we own it. No-op if externally owned or stopped.
pub async fn stop_embedded() -> EmbeddedStatus {
    // Wait briefly if a start is in flight.
    for _ in 0..10 {
        if !matches!(STATE.read().state, EmbeddedState::Starting) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    if STATE.read().externally_owned {
        tracing::info!("stop_embedded: listener is externally owned; no-op");
        return snapshot();
    }

    if let Some(handle) = HANDLE.lock().take() {
        drop(handle);
    }

    READY.store(false, Ordering::Release);
    CLAIMED.store(false, Ordering::Release);
    let mut st = STATE.write();
    st.state = EmbeddedState::Stopped;
    st.externally_owned = false;
    st.last_error = None;
    st.started_at_unix_ms = None;
    drop(st);
    snapshot()
}

/// Cheap snapshot. Does not perform any network I/O.
pub fn embedded_status() -> EmbeddedStatus {
    snapshot()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn pick_free_port() -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    }

    #[tokio::test]
    #[ignore = "uses process-global state; run with --test-threads=1 --ignored"]
    async fn start_then_stop_is_idempotent() {
        let port = pick_free_port();
        let a = start_embedded(port).await;
        assert!(matches!(a.state, EmbeddedState::Running));
        assert!(!a.externally_owned);
        let b = start_embedded(port).await;
        assert!(matches!(b.state, EmbeddedState::Running));
        let c = stop_embedded().await;
        assert!(matches!(c.state, EmbeddedState::Stopped));
    }

    #[tokio::test]
    #[ignore = "uses process-global state; run with --test-threads=1 --ignored"]
    async fn external_listener_is_adopted_and_not_killed() {
        let port = pick_free_port();
        let _listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        let a = start_embedded(port).await;
        assert!(matches!(a.state, EmbeddedState::Running));
        assert!(a.externally_owned);
        let b = stop_embedded().await;
        // External listener still alive — no-op stop.
        assert!(matches!(b.state, EmbeddedState::Running));
        assert!(b.externally_owned);
    }
}
