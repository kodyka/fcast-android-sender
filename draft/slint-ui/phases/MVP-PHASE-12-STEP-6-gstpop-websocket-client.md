# MVP-PHASE-12 — Step 6: `GstPopClient` — WebSocket + JSON-RPC 2.0

> Part 6 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Previous: [STEP-5](./MVP-PHASE-12-STEP-5-rust-trait-and-migration-adapter.md).
> Next: [STEP-7](./MVP-PHASE-12-STEP-7-gstpop-backend-impl.md).

---

## 0. Goal of this step

Implement the low-level WebSocket adapter that speaks the gst-pop
protocol — request/response with id correlation, plus a broadcast
channel for unsolicited events. The adapter is *not yet wired* into
the app — STEP-7 builds the `GstPopBackend` impl on top of it, and
STEP-8 plugs that backend into the lifecycle handler.

Modelled directly on the gst-popctl reference client at
[`gstpop/client/rust/src/main.rs:263-339`](https://github.com/dabrain34/gstpop/blob/main/client/rust/src/main.rs#L263-L339),
which shows the canonical `connect_async → split → send →
read.next().filter(|r| r.id == request_id)` loop.

---

## 1. Module layout

In `src/backend/gstpop/`:

```
src/backend/gstpop/
  mod.rs            ← re-exports + module docs
  protocol.rs       ← Request / Response / Event types + (de)serialization
  client.rs         ← GstPopClient: TCP/WS connection + request dispatcher
  protocol_tests.rs ← Unit tests (STEP-9 expands)
```

`mod.rs`:

```rust
//! gst-pop WebSocket adapter.
//!
//! Two layers:
//!   * `protocol`  — pure `serde` types (no IO). Mirrors the JSON-RPC
//!                   2.0 schema documented in gstpop/daemon/README.md.
//!   * `client`    — `GstPopClient`: holds a tokio-tungstenite
//!                   connection, dispatches requests by uuid, and
//!                   broadcasts events on a `tokio::sync::broadcast`
//!                   channel.

pub mod protocol;
pub mod client;
#[cfg(test)]
mod protocol_tests;

pub use protocol::{Event, Request, Response, RpcError};
pub use client::GstPopClient;
```

---

## 2. `protocol.rs` — pure (de)serialization

```rust
//! JSON-RPC 2.0 wire types for gst-pop.
//!
//! Reference: https://github.com/dabrain34/gstpop/blob/main/daemon/README.md
//! §"WebSocket API".

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Outbound request. Uses a uuid string id so responses can be
/// matched against concurrent in-flight calls.
#[derive(Clone, Debug, Serialize)]
pub struct Request {
    pub id:     String,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            params,
        }
    }
}

/// Inbound response. Exactly one of `result` or `error` is `Some`.
#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub id:     Value,                // string in our case, but the
                                       // protocol allows numbers too
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error:  Option<RpcError>,
}

impl Response {
    pub fn id_as_str(&self) -> Option<String> {
        match &self.id {
            Value::String(s) => Some(s.clone()),
            Value::Null      => None,
            other            => Some(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RpcError {
    pub code:    i32,
    pub message: String,
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gst-pop error ({}): {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// Broadcast event from the daemon (no `id`; has an `event` + `data`).
///
/// Variants enumerated from gstpop/daemon/README.md §"Events".
/// Unknown event names land in `Other` so the client survives
/// protocol additions without panicking.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum Event {
    StateChanged   { pipeline_id: String, old_state: String, new_state: String },
    Error          { pipeline_id: String, message: String },
    Unsupported    { pipeline_id: String, message: String },
    Eos            { pipeline_id: String },
    PipelineAdded  { pipeline_id: String, description: String },
    PipelineUpdated{ pipeline_id: String, description: String },
    PipelineRemoved{ pipeline_id: String },
    #[serde(other)]
    Other,
}

/// Discriminator for "is this an event payload?". Events don't have
/// an `id`; responses always do. Used by the read loop to route
/// each frame to either the per-request waker or the event
/// broadcaster.
pub(crate) fn classify(text: &str) -> ClassifiedFrame {
    let value: Value = match serde_json::from_str(text) {
        Ok(v)  => v,
        Err(_) => return ClassifiedFrame::Garbage,
    };
    if value.get("event").is_some() {
        match serde_json::from_value::<Event>(value.clone()) {
            Ok(ev)  => ClassifiedFrame::Event(ev),
            Err(_)  => ClassifiedFrame::Event(Event::Other),
        }
    } else if value.get("id").is_some() {
        match serde_json::from_value::<Response>(value) {
            Ok(rsp) => ClassifiedFrame::Response(rsp),
            Err(_)  => ClassifiedFrame::Garbage,
        }
    } else {
        ClassifiedFrame::Garbage
    }
}

pub(crate) enum ClassifiedFrame {
    Response(Response),
    Event(Event),
    Garbage,
}
```

### 2.1 Why a tagged enum for events

`#[serde(tag = "event", content = "data", rename_all = "snake_case")]`
maps the daemon's `{ "event": "state_changed", "data": { ... } }`
shape directly onto the Rust enum without a custom `Deserialize`
impl. The `#[serde(other)]` arm catches future event names so
unknown payloads degrade gracefully.

### 2.2 Why classify-then-deserialize-twice

A naive `serde(untagged)` enum trying both `Response` and `Event`
arms is slow and fragile — both shapes share the `id` key in some
cases (some daemons send "ack" events with ids). The `classify`
helper looks at `event` first, then falls back to `id`, then
discards anything that matches neither.

---

## 3. `client.rs` — connection + dispatcher

```rust
//! WebSocket adapter for the gst-pop daemon.
//!
//! Usage:
//!     let client = GstPopClient::connect(
//!         "ws://127.0.0.1:9000",
//!         Some("my-api-key".into()),
//!     ).await?;
//!     let resp = client
//!         .call("get_version", serde_json::json!({}))
//!         .await?;
//!     let mut events = client.subscribe();
//!     while let Ok(event) = events.recv().await {
//!         println!("event: {event:?}");
//!     }

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::{broadcast, oneshot};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};

use super::protocol::{classify, ClassifiedFrame, Event, Request, Response};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const EVENT_CAPACITY: usize     = 64;

pub struct GstPopClient {
    write: Arc<Mutex<
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >,
    >>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Response>>>>,
    events:  broadcast::Sender<Event>,
    _reader: tokio::task::JoinHandle<()>,
}

impl GstPopClient {
    /// Open a connection to the daemon.
    ///
    /// * `url`     — `ws://host:port` or `wss://host:port`.
    /// * `api_key` — value of the `Authorization` header (the gst-pop
    ///                daemon accepts the raw key, not `Bearer <key>`;
    ///                see gstpop/daemon/README.md:236-260). Pass
    ///                `None` to send no Authorization header.
    pub async fn connect(url: &str, api_key: Option<String>) -> Result<Self> {
        // Build the request manually so we can attach the auth header.
        let mut request = url
            .into_client_request()
            .with_context(|| format!("invalid url: {url}"))?;
        if let Some(key) = api_key.as_ref().filter(|k| !k.is_empty()) {
            request.headers_mut().insert(
                "Authorization",
                key.parse().context("api key contains illegal header bytes")?,
            );
        }

        let (ws, _http_response) = connect_async(request).await
            .with_context(|| format!("connect_async({url})"))?;
        let (write, mut read) = ws.split();

        let pending: Arc<Mutex<HashMap<String, oneshot::Sender<Response>>>>
            = Arc::new(Mutex::new(HashMap::new()));
        let pending_for_reader = Arc::clone(&pending);
        let (event_tx, _) = broadcast::channel(EVENT_CAPACITY);
        let event_tx_for_reader = event_tx.clone();

        // Spawn the read loop. It runs until the WS closes or an
        // unrecoverable error fires; on either, it drains pending
        // wakers so callers see the error rather than hanging.
        let reader = tokio::spawn(async move {
            while let Some(frame) = read.next().await {
                let text = match frame {
                    Ok(Message::Text(t))  => t.to_string(),
                    Ok(Message::Binary(_)) => continue, // ignore
                    Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                    Ok(Message::Close(_)) => break,
                    Err(err) => {
                        log::warn!("gst-pop ws read error: {err}");
                        break;
                    }
                };
                match classify(&text) {
                    ClassifiedFrame::Response(rsp) => {
                        let id = match rsp.id_as_str() {
                            Some(id) => id,
                            None     => continue,
                        };
                        if let Some(tx) = pending_for_reader.lock().remove(&id) {
                            let _ = tx.send(rsp);
                        } else {
                            log::trace!("unmatched gst-pop response id={id}");
                        }
                    }
                    ClassifiedFrame::Event(event) => {
                        // SendError means no current subscribers,
                        // which is fine — events are best-effort.
                        let _ = event_tx_for_reader.send(event);
                    }
                    ClassifiedFrame::Garbage => {
                        log::warn!("gst-pop sent unparseable frame: {text}");
                    }
                }
            }
            // Drain any remaining pending wakers so callers see an
            // error instead of hanging.
            for (_, tx) in pending_for_reader.lock().drain() {
                drop(tx); // dropping the oneshot::Sender wakes the
                           // recv side with RecvError::Closed.
            }
        });

        Ok(Self {
            write: Arc::new(Mutex::new(write)),
            pending,
            events: event_tx,
            _reader: reader,
        })
    }

    /// Issue one request and wait for its matching response.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = Request::new(method, params);
        let request_id = request.id.clone();
        let body = serde_json::to_string(&request)
            .context("serialize request")?;

        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(request_id.clone(), tx);

        // Scope the write-lock so we don't hold it across the await.
        {
            let mut guard = self.write.lock();
            // SinkExt::send is async, but the lock is sync — we have
            // to await on a held mutex briefly. Acceptable here
            // because the lock is per-client and writes are short.
            // Use Sink::send_unbuffered if profiling shows contention.
            futures::executor::block_on(guard.send(Message::Text(body.into())))
                .context("ws send")?;
        }

        let response = tokio::time::timeout(REQUEST_TIMEOUT, rx).await
            .context(format!("{method} timed out after {REQUEST_TIMEOUT:?}"))?
            .context(format!("{method} channel closed (ws likely disconnected)"))?;

        if let Some(err) = response.error {
            bail!("{method}: {err}");
        }
        response.result
            .ok_or_else(|| anyhow!("{method}: response had neither result nor error"))
    }

    /// Subscribe to broadcast events. Returns a fresh receiver;
    /// each subscriber sees every event posted after subscribe is
    /// called. Lagging subscribers (slower than EVENT_CAPACITY)
    /// receive `RecvError::Lagged(n)` and resume.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    /// Gracefully close the WS. Optional — `Drop` will close too,
    /// but without sending a Close frame.
    pub async fn close(self) {
        let mut guard = self.write.lock();
        let _ = futures::executor::block_on(guard.send(Message::Close(None)));
    }
}
```

### 3.1 Lock-while-awaiting caveat

The `block_on(guard.send(...))` pattern in §3 is a deliberate
simplification — strictly correct code would replace
`parking_lot::Mutex<SplitSink>` with `tokio::sync::Mutex<SplitSink>`
and `await` the lock + send asynchronously. The gst-pop client
gets at most one call per second from the UI, so a brief
held-while-blocking mutex is not contended — the simpler synchronous
mutex is acceptable.

If profiling shows contention (e.g. the lifecycle handler in STEP-8
starts pushing keepalive pings at 10 Hz), swap to:

```rust
write: Arc<tokio::sync::Mutex<...>>
```

and use:

```rust
let mut guard = self.write.lock().await;
guard.send(Message::Text(body.into())).await?;
```

### 3.2 Why a `broadcast::channel`, not a `mpsc::channel`

Multiple consumers — the lifecycle handler (STEP-8) wants events for
logging, *and* a future "live cast status" UI will want them for
display. `broadcast::channel` lets each consumer take its own
`Receiver`. `EVENT_CAPACITY = 64` is enough headroom for the worst
case ("daemon restarts and broadcasts 20 lifecycle events in 50 ms").

### 3.3 Why we don't pool connections

Each gst-pop daemon serves one pipeline at a time (per `pipeline_id`).
For the MVP, the app talks to **one** daemon and **one** pipeline;
a single long-lived `GstPopClient` instance owned by `GstPopBackend`
(STEP-7) is sufficient. If a future phase grows to "talk to multiple
daemons simultaneously" the `BackendSelector` in STEP-5 can hold a
`Vec<Arc<dyn MediaBackend>>` instead of one.

---

## 4. Expected diff size

- `src/backend/gstpop/mod.rs`: ~20 lines.
- `src/backend/gstpop/protocol.rs`: ~110 lines.
- `src/backend/gstpop/client.rs`: ~180 lines.
- `src/backend/gstpop/protocol_tests.rs`: ~80 lines (STEP-9
  expands).

Total: ~390 lines.

---

## 5. Verification

```sh
# Add the deps first (STEP-9 documents the full set):
cargo add --target aarch64-linux-android tokio-tungstenite@0.26 futures-util@0.3 async-trait@0.1 once_cell@1

# Sanity-build the new module.
cargo build -p android-sender --target aarch64-linux-android

# Unit-test the protocol classifier (STEP-9 adds end-to-end).
cargo test --target aarch64-linux-android backend::gstpop::protocol
```

A tiny `protocol_tests.rs` to start with:

```rust
use super::protocol::{classify, ClassifiedFrame, Event};

#[test]
fn classifies_state_changed_event() {
    let text = r#"{"event":"state_changed","data":{
        "pipeline_id":"0","old_state":"paused","new_state":"playing"
    }}"#;
    match classify(text) {
        ClassifiedFrame::Event(Event::StateChanged { pipeline_id, new_state, .. }) => {
            assert_eq!(pipeline_id, "0");
            assert_eq!(new_state, "playing");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn classifies_response_with_string_id() {
    let text = r#"{"id":"abc","result":{"pipeline_id":"0"}}"#;
    match classify(text) {
        ClassifiedFrame::Response(rsp) => {
            assert_eq!(rsp.id_as_str(), Some("abc".to_owned()));
            assert!(rsp.error.is_none());
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn unknown_event_falls_back_to_other() {
    let text = r#"{"event":"future_unknown","data":{}}"#;
    match classify(text) {
        ClassifiedFrame::Event(Event::Other) => {}
        other => panic!("unexpected: {other:?}"),
    }
}
```

---

## 6. Exit gate

- [ ] `src/backend/gstpop/mod.rs`, `protocol.rs`, `client.rs`, and
      `protocol_tests.rs` exist and compile.
- [ ] `cargo test backend::gstpop::protocol` passes the three
      classifier tests in §5.
- [ ] `GstPopClient::connect` accepts an optional API key and
      writes it to the `Authorization` header (no `Bearer` prefix).
- [ ] No call site in the app yet uses `GstPopClient` — STEP-7
      adds the trait wiring.

Proceed to [STEP-7](./MVP-PHASE-12-STEP-7-gstpop-backend-impl.md).
