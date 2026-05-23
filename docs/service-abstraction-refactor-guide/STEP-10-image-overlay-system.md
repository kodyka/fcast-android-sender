# STEP 10 — Image Overlay System

**Phase:** 4 (Enhanced SRT Source Handling)
**New file:** `src/overlay/mod.rs`

---

## Goal

Create an image overlay manager that can compose one or more overlay images
on top of SRT video sources, with configurable position, size, alpha, and
z-order.

---

## 1. Define the overlay model

```rust
// src/overlay/mod.rs

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

pub type OverlayId = String;

/// Position and size of an overlay image on the canvas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverlayRect {
    /// X offset from canvas left (pixels).
    pub x: i32,
    /// Y offset from canvas top (pixels).
    pub y: i32,
    /// Width in pixels (0 = use original image width).
    pub width: u32,
    /// Height in pixels (0 = use original image height).
    pub height: u32,
}

impl Default for OverlayRect {
    fn default() -> Self {
        Self { x: 0, y: 0, width: 0, height: 0 }
    }
}

/// Configuration for a single overlay image.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverlayConfig {
    pub id: OverlayId,
    /// Source SRT slot this overlay is attached to.
    pub slot_id: String,
    /// Whether this overlay is currently rendered.
    pub visible: bool,
    /// File path or URL of the image.
    pub source: OverlaySource,
    /// Position and size on the composition canvas.
    pub rect: OverlayRect,
    /// Opacity: 0.0 (fully transparent) .. 1.0 (fully opaque).
    pub alpha: f64,
    /// Stacking order relative to other overlays on the same slot.
    pub z_order: i32,
}

/// Where the overlay image comes from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlaySource {
    File(PathBuf),
    Url(String),
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            slot_id: String::new(),
            visible: true,
            source: OverlaySource::File(PathBuf::new()),
            rect: OverlayRect::default(),
            alpha: 1.0,
            z_order: 10,
        }
    }
}
```

## 2. Overlay manager

```rust
/// Manages overlay images across all SRT source slots.
pub struct OverlayManager {
    overlays: Arc<RwLock<HashMap<OverlayId, OverlayConfig>>>,
}

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            overlays: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add or update an overlay.
    pub fn upsert(&self, config: OverlayConfig) {
        self.overlays.write().insert(config.id.clone(), config);
    }

    /// Remove an overlay by ID.
    pub fn remove(&self, id: &str) {
        self.overlays.write().remove(id);
    }

    /// All overlays attached to a specific SRT source slot, sorted by z-order.
    pub fn overlays_for_slot(&self, slot_id: &str) -> Vec<OverlayConfig> {
        let mut result: Vec<OverlayConfig> = self
            .overlays
            .read()
            .values()
            .filter(|o| o.slot_id == slot_id && o.visible)
            .cloned()
            .collect();
        result.sort_by_key(|o| o.z_order);
        result
    }

    /// All overlays (for persistence).
    pub fn all(&self) -> Vec<OverlayConfig> {
        self.overlays.read().values().cloned().collect()
    }
}
```

## 3. Image loading helper

```rust
/// Load an overlay image into a GStreamer-compatible RGBA buffer.
/// This is called during pipeline construction to feed the
/// `gdkpixbufoverlay` or `imagefreeze` element.
pub fn load_image_file(path: &std::path::Path) -> Result<Vec<u8>> {
    let data = std::fs::read(path)
        .with_context(|| format!("read overlay image {}", path.display()))?;
    // Decode with the `image` crate or GdkPixbuf.
    // For the guide we show the signature — implementation depends
    // on which image library is already in the dependency tree.
    //
    // If using the `image` crate:
    //   let img = image::load_from_memory(&data)?;
    //   let rgba = img.to_rgba8();
    //   Ok(rgba.into_raw())
    //
    // If staying pure-GStreamer, create a `gst::Pipeline` snippet:
    //   filesrc ! decodebin ! videoconvert ! video/x-raw,format=RGBA ! appsink
    Ok(data) // placeholder
}

/// Download an image from a URL into a temp file, then load it.
pub async fn load_image_url(url: &str) -> Result<Vec<u8>> {
    // Use reqwest or the system curl.
    // For now, this is a placeholder showing the intended API.
    anyhow::bail!("URL overlay loading not yet implemented: {url}")
}
```

## 4. Register the module

```rust
// src/lib.rs
pub mod overlay;
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `src/overlay/mod.rs` with the code above | new file |
| 2 | Add `pub mod overlay;` to `src/lib.rs` | line ~39 |
| 3 | Instantiate `OverlayManager` alongside `SrtSourceManager` in init | `lib.rs` |
| 4 | Decide on image loading library (`image` crate vs GdkPixbuf vs pure GStreamer) | Cargo.toml |
| 5 | If `image` crate is chosen, add `image = "0.25"` to `[dependencies]` | Cargo.toml |
| 6 | Verify `cargo check` passes | terminal |

---

## Notes

* The `OverlayManager` is deliberately decoupled from GStreamer.  The
  actual GStreamer integration happens in STEP 11 where overlay configs
  are translated into pipeline elements.
* Multiple overlays per slot are supported (e.g. a logo + a lower-third
  banner on the same SRT source).
* Persistence of overlay configs can piggyback on the existing
  `StoredBackendConfig` save/load pattern or use a separate
  `overlays.json` file.
