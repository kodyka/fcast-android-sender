//! Image overlay composition for SRT video sources.
//! Manages overlay images (position, size, alpha, z-order) and
//! translates them into GStreamer compositor pads.

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
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
}

/// Where the overlay image comes from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlaySource {
    File(PathBuf),
    Url(String),
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

impl Default for OverlayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Load an overlay image file into raw bytes for GStreamer pipeline use.
pub fn load_image_file(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .with_context(|| format!("read overlay image {}", path.display()))
}

/// Download an image from a URL (placeholder — not yet implemented).
pub async fn load_image_url(url: &str) -> Result<Vec<u8>> {
    anyhow::bail!("URL overlay loading not yet implemented: {url}")
}
