use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};

use gstpop::{
    gst::{create_event_channel, PipelineManager},
    server::{ServerConfig, ServerHandle},
};

// Hardware atomic — safe to use across separate tokio runtimes (two android_main threads).
static CLAIMED: AtomicBool = AtomicBool::new(false);
// READY is set to true only after the server is listening and the handle is stored.
static READY: AtomicBool = AtomicBool::new(false);

// Keep the ServerHandle alive for the process lifetime (its task is aborted on drop).
static HANDLE: std::sync::Mutex<Option<ServerHandle>> = std::sync::Mutex::new(None);

/// Start the embedded gst-pop WebSocket server on 127.0.0.1:{port} if not already running.
/// Race-safe across multiple tokio runtimes via atomic compare-and-swap.
pub async fn ensure_started(port: u16) -> Result<()> {
    // Fast path: already fully running.
    if READY.load(Ordering::Acquire) {
        return Ok(());
    }

    // If something is already listening on the port (a manually-started
    // gst-pop daemon, the CI smoke-test daemon running in Docker, etc.),
    // there is no point starting a second embedded server on top of it —
    // the bind would just fail with EADDRINUSE. Treat the existing
    // listener as our gst-pop and mark the slot claimed/ready.
    if probe_port_open(port).await {
        CLAIMED.store(true, Ordering::Release);
        READY.store(true, Ordering::Release);
        tracing::info!(
            "External gst-pop server already listening on 127.0.0.1:{port}; \
             skipping embedded startup"
        );
        return Ok(());
    }

    // Race to be the one that starts the server. Only one caller wins.
    if CLAIMED.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        // We won — start the server.
        match start_server(port).await {
            Ok(handle) => {
                *HANDLE.lock().unwrap() = Some(handle);
                READY.store(true, Ordering::Release);
                tracing::info!("Embedded gst-pop server running on 127.0.0.1:{port}");
                Ok(())
            }
            Err(e) => {
                // Reset so a future probe attempt can retry.
                CLAIMED.store(false, Ordering::Release);
                Err(e)
            }
        }
    } else {
        // We lost the race — wait for the winner to finish binding the port.
        wait_for_port(port).await
    }
}

/// Quick TCP-connect probe: returns true if `127.0.0.1:port` accepts a
/// connection within ~200ms.
async fn probe_port_open(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    let connect = tokio::net::TcpStream::connect(&addr);
    matches!(
        tokio::time::timeout(std::time::Duration::from_millis(200), connect).await,
        Ok(Ok(_))
    )
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

    // Wait until the WebSocket task is actually listening before returning.
    wait_for_port(port).await?;

    Ok(handle)
}

/// Returns true if `url` targets the local loopback interface.
pub fn is_localhost(url: &str) -> bool {
    url.contains("127.0.0.1") || url.contains("localhost") || url.contains("[::1]")
}

/// Extract port from a WebSocket URL like "ws://127.0.0.1:9000".
pub fn url_port(url: &str) -> u16 {
    url.rsplit(':')
        .next()
        .and_then(|s| s.trim_end_matches('/').parse::<u16>().ok())
        .unwrap_or(9000)
}

/// Poll until the TCP port accepts connections or timeout after 2s.
async fn wait_for_port(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(&addr).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    anyhow::bail!("embedded gst-pop server did not start on port {port} within 2s")
}
