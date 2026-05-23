# STEP 09 — SRT Source Manager

**Phase:** 4 (Enhanced SRT Source Handling)
**New file:** `src/srt/mod.rs`

---

## Goal

Create a dedicated SRT source manager that supports more than two sources,
monitors connection health, and handles automatic reconnection.

---

## 1. Define the SRT source model

```rust
// src/srt/mod.rs

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

/// Unique identifier for an SRT source slot.
pub type SlotId = String;

/// Health state of an SRT connection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SrtConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Per-slot configuration persisted across sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SrtSourceConfig {
    pub slot_id: SlotId,
    pub enabled: bool,
    pub uri: String,
    pub latency_ms: u32,
    pub stream_id: String,

    // Mixing parameters
    pub mix_alpha: f64,
    pub mix_zorder: i32,
    pub mix_volume: f64,

    // Reconnection policy
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,
}

fn default_max_retries() -> u32 { 5 }
fn default_retry_delay_ms() -> u64 { 2000 }

impl Default for SrtSourceConfig {
    fn default() -> Self {
        Self {
            slot_id: String::new(),
            enabled: true,
            uri: String::new(),
            latency_ms: 2000,
            stream_id: String::new(),
            mix_alpha: 1.0,
            mix_zorder: 0,
            mix_volume: 1.0,
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay_ms(),
        }
    }
}
```

## 2. Define the runtime state per source

```rust
/// Live runtime state for one SRT source.
#[derive(Clone, Debug)]
pub struct SrtSourceState {
    pub config: SrtSourceConfig,
    pub connection: SrtConnectionState,
    pub last_error: Option<String>,
    pub retry_count: u32,
    pub connected_since: Option<std::time::Instant>,
    pub bytes_received: u64,
}
```

## 3. Implement the manager

```rust
/// Manages a dynamic set of SRT source slots.
pub struct SrtSourceManager {
    slots: Arc<RwLock<HashMap<SlotId, SrtSourceState>>>,
    state_tx: watch::Sender<HashMap<SlotId, SrtSourceState>>,
    state_rx: watch::Receiver<HashMap<SlotId, SrtSourceState>>,
}

impl SrtSourceManager {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(HashMap::new());
        Self {
            slots: Arc::new(RwLock::new(HashMap::new())),
            state_tx: tx,
            state_rx: rx,
        }
    }

    /// Add or update a source slot.
    pub fn upsert_slot(&self, config: SrtSourceConfig) {
        let mut slots = self.slots.write();
        let state = slots.entry(config.slot_id.clone()).or_insert_with(|| {
            SrtSourceState {
                config: config.clone(),
                connection: SrtConnectionState::Disconnected,
                last_error: None,
                retry_count: 0,
                connected_since: None,
                bytes_received: 0,
            }
        });
        state.config = config;
        let snapshot = slots.clone();
        drop(slots);
        let _ = self.state_tx.send(snapshot);
    }

    /// Remove a slot.
    pub fn remove_slot(&self, slot_id: &str) {
        let mut slots = self.slots.write();
        slots.remove(slot_id);
        let snapshot = slots.clone();
        drop(slots);
        let _ = self.state_tx.send(snapshot);
    }

    /// Get a watch receiver for UI updates.
    pub fn subscribe(&self) -> watch::Receiver<HashMap<SlotId, SrtSourceState>> {
        self.state_rx.clone()
    }

    /// Snapshot of all current slot states.
    pub fn snapshot(&self) -> HashMap<SlotId, SrtSourceState> {
        self.slots.read().clone()
    }
}
```

## 4. Add health monitoring with auto-reconnect

```rust
impl SrtSourceManager {
    /// Spawn a background task that monitors each enabled slot and
    /// attempts reconnection when the connection drops.
    pub fn spawn_health_monitor(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                let slots = manager.slots.read().clone();
                for (slot_id, state) in &slots {
                    if !state.config.enabled {
                        continue;
                    }
                    match state.connection {
                        SrtConnectionState::Error | SrtConnectionState::Disconnected => {
                            if state.retry_count < state.config.max_retries {
                                tracing::info!(
                                    slot_id,
                                    retry = state.retry_count,
                                    "SRT auto-reconnect attempt"
                                );
                                manager.attempt_reconnect(slot_id).await;
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
    }

    async fn attempt_reconnect(&self, slot_id: &str) {
        let mut slots = self.slots.write();
        if let Some(state) = slots.get_mut(slot_id) {
            state.connection = SrtConnectionState::Reconnecting;
            state.retry_count += 1;
        }
        let snapshot = slots.clone();
        drop(slots);
        let _ = self.state_tx.send(snapshot);

        // Actual reconnection would be done by tearing down and
        // recreating the GStreamer source node via the migration
        // runtime or gst-pop backend.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // After attempting, call into the backend to check if
        // the source came up.  On success:
        //   state.connection = SrtConnectionState::Connected;
        //   state.retry_count = 0;
        // On failure:
        //   state.connection = SrtConnectionState::Error;
        //   state.last_error = Some(err.to_string());
    }
}
```

## 5. Register the module

```rust
// src/lib.rs  (add near the other module declarations)
pub mod srt;
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `src/srt/mod.rs` with the code above | new file |
| 2 | Add `pub mod srt;` to `src/lib.rs` | line ~38 |
| 3 | Instantiate `SrtSourceManager` in the app startup path | `lib.rs` main init |
| 4 | Spawn the health monitor task alongside the existing 1Hz poller | `lifecycle.rs` |
| 5 | Wire `subscribe()` output to push SRT states to Slint Bridge | new callback |
| 6 | Verify `cargo check` passes | terminal |

---

## Notes

* The existing `SrtSource` Slint struct (in `bridge.slint`) remains the
  UI-facing model.  `SrtSourceState` is the Rust runtime model.  A
  conversion function maps one to the other for UI updates.
* Currently the app hard-codes Source A and Source B.  This manager
  supports N slots by using a `HashMap<SlotId, ...>`.  The UI in STEP 12
  will render a dynamic list.
