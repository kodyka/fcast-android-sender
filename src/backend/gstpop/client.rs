use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};

use super::protocol::{classify, ClassifiedFrame, Event, Request, Response};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const EVENT_CAPACITY: usize = 64;

type WsWrite = futures_util::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

pub struct GstPopClient {
    write: Arc<AsyncMutex<WsWrite>>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Response>>>>,
    events: broadcast::Sender<Event>,
    _reader: tokio::task::JoinHandle<()>,
}

impl GstPopClient {
    pub async fn connect(url: &str, api_key: Option<String>) -> Result<Self> {
        let mut request = url
            .into_client_request()
            .with_context(|| format!("invalid url: {url}"))?;
        if let Some(key) = api_key.as_ref().filter(|value| !value.is_empty()) {
            request.headers_mut().insert(
                "Authorization",
                key.parse()
                    .context("api key contains illegal header bytes")?,
            );
        }

        let (ws, _) = connect_async(request)
            .await
            .with_context(|| format!("connect_async({url})"))?;
        let (write, mut read) = ws.split();

        let pending: Arc<Mutex<HashMap<String, oneshot::Sender<Response>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_reader = Arc::clone(&pending);
        let (event_tx, _) = broadcast::channel(EVENT_CAPACITY);
        let event_tx_reader = event_tx.clone();

        let reader = tokio::spawn(async move {
            while let Some(frame) = read.next().await {
                let text = match frame {
                    Ok(Message::Text(text)) => text.to_string(),
                    Ok(Message::Binary(_)) => continue,
                    Ok(Message::Ping(_) | Message::Pong(_) | Message::Close(_)) => continue,
                    Err(err) => {
                        log::warn!("gst-pop ws read error: {err}");
                        break;
                    }
                    _ => continue,
                };

                match classify(&text) {
                    ClassifiedFrame::Response(response) => {
                        let Some(id) = response.id_as_str() else {
                            continue;
                        };
                        if let Some(tx) = pending_reader.lock().remove(&id) {
                            let _ = tx.send(response);
                        }
                    }
                    ClassifiedFrame::Event(event) => {
                        let _ = event_tx_reader.send(event);
                    }
                    ClassifiedFrame::Garbage => {
                        log::warn!("gst-pop sent unparseable frame: {text}");
                    }
                }
            }

            for (_, tx) in pending_reader.lock().drain() {
                drop(tx);
            }
        });

        Ok(Self {
            write: Arc::new(AsyncMutex::new(write)),
            pending,
            events: event_tx,
            _reader: reader,
        })
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = Request::new(method, params);
        let request_id = request.id.clone();
        let body = serde_json::to_string(&request).context("serialize request")?;

        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(request_id.clone(), tx);

        {
            let mut guard = self.write.lock().await;
            guard
                .send(Message::Text(body.into()))
                .await
                .context("ws send")?;
        }

        let response = match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                self.pending.lock().remove(&request_id);
                bail!("{method}: channel closed (ws likely disconnected)");
            }
            Err(_) => {
                self.pending.lock().remove(&request_id);
                bail!("{method} timed out after {REQUEST_TIMEOUT:?}");
            }
        };

        if let Some(err) = response.error {
            bail!("{method}: {err}");
        }

        response
            .result
            .ok_or_else(|| anyhow!("{method}: response had neither result nor error"))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub async fn close(&self) {
        let mut guard = self.write.lock().await;
        let _ = guard.send(Message::Close(None)).await;
    }
}
