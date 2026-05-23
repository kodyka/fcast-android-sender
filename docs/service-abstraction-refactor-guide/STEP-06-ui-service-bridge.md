# STEP 06 — UI Service Bridge

**Phase:** 3 (Independent UI Layer)
**New file:** `ui/state/service_bridge.slint`

---

## Goal

Create a service-agnostic UI bridge that pages can consume without
depending on the concrete `Bridge.gstpop-*` or `Bridge.media-backend-*`
properties.  This allows the UI to function uniformly regardless of which
services are active.

---

## 1. Define the service bridge global

```slint
// ui/state/service_bridge.slint

import { MixerState } from "../bridge.slint";

/// A single service's UI-facing state.
export struct ServiceEntry {
    id:          string,   // "gstpop" | "migration"
    label:       string,   // Human-readable name
    enabled:     bool,
    running:     bool,
    healthy:     bool,
    status-text: string,
    error-text:  string,
}

/// Generic media operation the UI can request without knowing which
/// backend will fulfil it.
export enum MediaOp {
    start,
    stop,
    probe,
}

export global ServiceBridge {
    // ── Per-service status (Rust -> Slint) ────────────────────────────
    in property <[ServiceEntry]> services: [];

    // ── Aggregate health (Rust -> Slint) ──────────────────────────────
    // True when at least one service is running and healthy.
    in property <bool> any-service-ready: false;

    // ── Generic commands (Slint -> Rust) ──────────────────────────────
    callback request-service-op(id: string, op: MediaOp);
    // e.g. ServiceBridge.request-service-op("gstpop", MediaOp.start)

    // ── Media source / destination operations ─────────────────────────
    // These abstract over the concrete backend so pages never call
    // Bridge.start-mixer-cast() directly.
    callback start-stream();
    callback stop-stream();
    callback apply-source-config(slot-id: string, params: string);
}
```

## 2. Register the global

```slint
// ui/state/index.slint  (add)
import { ServiceBridge } from "service_bridge.slint";
export { ServiceBridge }
```

```slint
// ui/main.slint  (add to the import + export lines)
import { ..., ServiceBridge } from "state/index.slint";
export { ..., ServiceBridge }
```

## 3. Rust-side population

```rust
// src/backend/lifecycle.rs  (or a new src/service/bridge.rs)

use crate::{ServiceBridge, ServiceEntry};

/// Push the current service states into the Slint ServiceBridge model.
fn push_services(weak: &slint::Weak<MainWindow>, services: &[ServiceStatus]) {
    let entries: Vec<ServiceEntry> = services
        .iter()
        .map(|s| ServiceEntry {
            id:          s.id.clone().into(),
            label:       s.label.clone().into(),
            enabled:     s.enabled,
            running:     s.running,
            healthy:     s.healthy,
            status_text: s.status_text.clone().into(),
            error_text:  s.error_text.clone().into(),
        })
        .collect();

    let any_ready = entries.iter().any(|e| e.running && e.healthy);
    let model = std::rc::Rc::new(slint::VecModel::from(entries));

    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<ServiceBridge>();
        bridge.set_services(model.into());
        bridge.set_any_service_ready(any_ready);
    });
}
```

Wire the command callback:

```rust
let sb = ui.global::<ServiceBridge>();
sb.on_request_service_op(move |id, op| {
    let id = id.to_string();
    let weak = weak.clone();
    tokio::spawn(async move {
        // Look up the ServiceManager by id and call start/stop/status
        match op {
            MediaOp::Start => { /* service_registry.get(&id).start().await */ }
            MediaOp::Stop  => { /* service_registry.get(&id).stop().await  */ }
            MediaOp::Probe => { /* service_registry.get(&id).status().await */ }
        }
        // Then push_services(...)
    });
});
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `ui/state/service_bridge.slint` | new file |
| 2 | Register in `state/index.slint` and re-export from `main.slint` | existing files |
| 3 | Implement `push_services()` in Rust | `lifecycle.rs` or new |
| 4 | Wire `on_request_service_op` to the `ServiceManager` registry | Rust callback |
| 5 | Wire `on_start_stream` / `on_stop_stream` delegating to current backend | Rust callback |

---

## Notes

* Pages that currently call `Bridge.start-gstpop-service()` should migrate
  to `ServiceBridge.request-service-op("gstpop", MediaOp.start)`.  This
  can happen incrementally — both paths can coexist during transition.
* `any-service-ready` is used by STEP 08 to conditionally show/hide
  service-dependent UI sections.
