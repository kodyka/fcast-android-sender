# STEP 16 — Documentation & Examples

**Phase:** 6 (Documentation and Examples)
**Modified files:** `docs/`, `README.md`

---

## Goal

Update the README with the new service architecture, create a service
configuration guide, document SRT source and overlay usage, and provide
code examples for extending the system.

---

## 1. Update README.md

Add a new section after "Repository layout":

```markdown
## Architecture overview

### Service abstraction layer

The app uses a `ServiceManager` trait to abstract service lifecycle:

```text
┌───────────────────────────────────────────┐
│               ServiceManager              │
│  start() / stop() / status()              │
├──────────────────┬────────────────────────┤
│  GstPopService   │  MigrationService      │
│  Manager         │  Manager               │
│  (Android/       │  (in-process           │
│   Embedded/      │   runtime)             │
│   External)      │                        │
└──────────────────┴────────────────────────┘
```

Each service can be independently enabled/disabled via the Service
Configuration page.  The UI functions correctly with zero, one, or both
services running.

### SRT sources & overlays

The mixer supports N SRT source slots (extending the original A/B pair).
Each slot can have multiple image overlays composited on top via GStreamer
`compositor` pads.

```text
SRT Source 1 ──┐
               ├── compositor ──► video output
SRT Source 2 ──┤     ▲
               │     │
  Overlay 1 ───┤     │
  Overlay 2 ───┘     │
                      │
  Audio mixer ────────┘ (separate audio path)
```
```

## 2. Service configuration guide

Create `docs/service-configuration.md`:

```markdown
# Service Configuration Guide

## Overview

fcast-android-sender supports two media services:

| Service | Description | Default mode |
|---------|-------------|-------------|
| **gst-pop** | GStreamer daemon for pipeline management | Android Service |
| **Migration** | In-process media engine | Embedded |

## Configuration file

Service settings are stored in `backend.json` alongside the existing
backend configuration:

```json
{
  "kind": "migration",
  "gstpop_url": "ws://127.0.0.1:9000",
  "gstpop_api_key": null,
  "gstpop_pipeline_id": "0",
  "gstpop_service": {
    "enabled": true,
    "auto_start": true,
    "mode": "android-service"
  },
  "migration_service": {
    "enabled": true,
    "auto_start": true,
    "mode": "embedded"
  },
  "auto_start_services": true,
  "service_mode": "embedded"
}
```

## Service modes

| Mode | Description |
|------|-------------|
| `embedded` | Run inside the app process |
| `android-service` | Managed Android foreground Service |
| `external` | Connect to a user-supplied daemon |

## UI access

Open **Settings > Service Configuration** to toggle services and
change modes at runtime.

## Running without services

The app can run with all services disabled.  The UI will show a
"No service running" notice with a link to the configuration page.
Media features (casting, mixing) are unavailable until at least one
service is started.
```

## 3. SRT and overlay usage guide

Create `docs/srt-and-overlays.md`:

```markdown
# SRT Sources & Image Overlays

## Adding SRT sources

1. Open **Settings > SRT Sources & Overlays**
2. Tap **+ Add SRT Source**
3. Enter the SRT URL (e.g. `srt://relay.example:9710?mode=caller`)
4. Adjust latency (default: 2000 ms)
5. Optionally set a stream ID

## Adding image overlays

1. In the SRT source card, tap **+ Add Overlay**
2. Enter the image file path or URL
3. Adjust position (X, Y), size (Width, Height), alpha, and z-order
4. The composition preview shows a schematic layout

## Overlay parameters

| Parameter | Range | Description |
|-----------|-------|-------------|
| X, Y | -1920..3840 | Position offset from top-left |
| Width, Height | 0..1920 | 0 = use original image size |
| Alpha | 0.0..1.0 | Opacity |
| Z-order | 0..99 | Higher = drawn on top |

## Auto-reconnection

SRT sources automatically reconnect on connection loss:
- Default: 5 retries with 2-second delay
- Configurable per-source via `max_retries` and `retry_delay_ms`
- Connection state is shown in the UI (Disconnected / Connecting / Connected / Reconnecting / Error)
```

## 4. Adding a new service backend (example)

Create `docs/examples/new-service-backend.md`:

```markdown
# Example: Adding a New Service Backend

This example shows how to add a hypothetical "WebRTC Service" backend.

## 1. Implement ServiceManager

```rust
// src/webrtc/service.rs

use crate::service::{ServiceManager, ServiceOptions, ServiceStatus};

pub struct WebRtcServiceManager {
    options: parking_lot::RwLock<ServiceOptions>,
}

impl WebRtcServiceManager {
    pub fn new(options: ServiceOptions) -> Self {
        Self {
            options: parking_lot::RwLock::new(options),
        }
    }
}

#[async_trait::async_trait]
impl ServiceManager for WebRtcServiceManager {
    fn name(&self) -> &str { "webrtc" }

    fn options(&self) -> &ServiceOptions {
        self.options.read().clone()
    }

    fn set_options(&mut self, options: ServiceOptions) {
        *self.options.write() = options;
    }

    async fn start(&self) -> anyhow::Result<ServiceStatus> {
        // Your start logic here
        Ok(ServiceStatus {
            running: true,
            healthy: true,
            status_text: "WebRTC service running".into(),
            error_text: String::new(),
        })
    }

    async fn stop(&self) -> anyhow::Result<ServiceStatus> {
        Ok(ServiceStatus {
            running: false,
            healthy: true,
            status_text: "WebRTC service stopped".into(),
            error_text: String::new(),
        })
    }

    async fn status(&self) -> anyhow::Result<ServiceStatus> {
        Ok(ServiceStatus {
            running: true,
            healthy: true,
            status_text: "ok".into(),
            error_text: String::new(),
        })
    }
}
```

## 2. Register in the service registry

```rust
// In your app init code:
service_registry.insert("webrtc", Box::new(WebRtcServiceManager::new(opts)));
```

## 3. Add to BackendKind (if it's a new media backend)

```rust
// src/backend/kind.rs
pub enum BackendKind {
    Migration,
    GstPop,
    WebRtc,  // new
}
```

## 4. Add UI toggle

The ServiceBridge automatically shows any registered service in the
Service Configuration page — no Slint changes needed.
```

## 5. Custom overlay example

Create `docs/examples/custom-overlay.md`:

```markdown
# Example: Custom Overlay Implementation

## Adding a text overlay (clock / timer)

Instead of a static image, create a dynamic text overlay using
GStreamer's `textoverlay` element:

```rust
// In src/overlay/mod.rs (extend OverlaySource enum)

pub enum OverlaySource {
    File(PathBuf),
    Url(String),
    DynamicText {
        template: String,   // e.g. "%H:%M:%S"
        font: String,       // e.g. "Sans Bold 24"
    },
}
```

The mixer integration (STEP 11) would handle `DynamicText` by
creating a `textoverlay` element instead of `imagefreeze`:

```rust
OverlaySource::DynamicText { template, font } => {
    let textoverlay = Self::make_element("textoverlay", None)?;
    textoverlay.set_property("text", &template);
    textoverlay.set_property("font-desc", &font);
    // ... connect to compositor pad ...
}
```
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Update `README.md` with architecture overview | existing file |
| 2 | Create `docs/service-configuration.md` | new file |
| 3 | Create `docs/srt-and-overlays.md` | new file |
| 4 | Create `docs/examples/new-service-backend.md` | new file |
| 5 | Create `docs/examples/custom-overlay.md` | new file |
| 6 | Review all doc links from README | terminal: check dead links |

---

## Notes

* All documentation follows the existing `docs/` convention of one topic
  per file.
* Code examples are self-contained and reference the exact module paths
  from the refactored codebase.
* The architecture diagram uses ASCII art for maximum portability (no
  external image dependencies).
