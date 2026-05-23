# STEP 11 — Mixer Overlay Integration

**Phase:** 4 (Enhanced SRT Source Handling)
**Extended file:** `src/migration/nodes/mixer.rs`

---

## Goal

Add image overlay composition to the mixer's GStreamer pipeline so that
overlay images from `OverlayManager` are composited onto the video output
alongside the regular SRT source slots.

---

## 1. Overlay GStreamer strategy

GStreamer provides several elements for image overlay:

| Element | Use-case |
|---------|----------|
| `gdkpixbufoverlay` | Simple static image overlay with x/y/alpha |
| `compositor` (extra pad) | Full compositing with per-pad z-order |
| `imagefreeze` + `compositor` pad | Convert a single image into a video stream, then composite |

**Recommended approach:** Use extra `compositor` pads on the existing video
mixer.  Each overlay image is fed through `imagefreeze` to produce a
continuous video stream, then connected to a new `compositor` sink pad
with position / size / alpha / z-order properties.

## 2. Add overlay pad creation to `MixerNode`

```rust
// src/migration/nodes/mixer.rs  (add to MixerNode impl)

use crate::overlay::OverlayConfig;

impl MixerNode {
    /// Add an image overlay as an extra compositor pad.
    ///
    /// The pipeline segment looks like:
    ///   filesrc ! decodebin ! videoconvert ! videoscale
    ///   ! video/x-raw,width=W,height=H ! imagefreeze ! compositor.sink_N
    ///
    /// Returns the GStreamer element names for later teardown.
    pub fn add_overlay(
        &mut self,
        overlay: &OverlayConfig,
    ) -> Result<Vec<String>, String> {
        let live = self.live_pipeline.as_mut().ok_or("mixer not running")?;
        let compositor = live
            .video_mixer
            .as_ref()
            .ok_or("no video compositor in mixer")?;

        let pipeline = &live.pipeline;

        // 1. Create elements
        let src_name = format!("overlay-src-{}", overlay.id);
        let freeze_name = format!("overlay-freeze-{}", overlay.id);
        let convert_name = format!("overlay-convert-{}", overlay.id);
        let scale_name = format!("overlay-scale-{}", overlay.id);

        let filesrc = Self::make_element("filesrc", Some(&src_name))?;
        let decodebin = Self::make_element("decodebin", None)?;
        let videoconvert = Self::make_element("videoconvert", Some(&convert_name))?;
        let videoscale = Self::make_element("videoscale", Some(&scale_name))?;
        let imagefreeze = Self::make_element("imagefreeze", Some(&freeze_name))?;

        // Set file path
        match &overlay.source {
            crate::overlay::OverlaySource::File(path) => {
                filesrc.set_property(
                    "location",
                    path.to_str().unwrap_or_default(),
                );
            }
            crate::overlay::OverlaySource::Url(_url) => {
                // For URL sources, download first and use the temp path.
                return Err("URL overlay sources not yet supported in pipeline".into());
            }
        }

        // 2. Add to pipeline
        pipeline
            .add_many([&filesrc, &decodebin, &videoconvert, &videoscale, &imagefreeze])
            .map_err(|e| format!("add overlay elements: {e}"))?;

        // 3. Link static elements
        //    filesrc -> decodebin (dynamic pad, handled below)
        gst::Element::link(&filesrc, &decodebin)
            .map_err(|e| format!("link filesrc->decodebin: {e}"))?;
        gst::Element::link_many([&videoconvert, &videoscale, &imagefreeze])
            .map_err(|e| format!("link convert->scale->freeze: {e}"))?;

        // 4. Handle decodebin's dynamic pad
        let convert_weak = videoconvert.downgrade();
        decodebin.connect_pad_added(move |_element, src_pad| {
            let Some(convert) = convert_weak.upgrade() else { return };
            let sink_pad = convert.static_pad("sink").expect("videoconvert sink");
            if !sink_pad.is_linked() {
                let _ = src_pad.link(&sink_pad);
            }
        });

        // 5. Request a new sink pad on the compositor
        let templ = compositor
            .pad_template("sink_%u")
            .ok_or("compositor missing sink_%u template")?;
        let comp_pad = compositor
            .request_pad(&templ, None, None)
            .ok_or("failed to request compositor sink pad")?;

        // Set pad properties
        comp_pad.set_property("xpos", overlay.rect.x);
        comp_pad.set_property("ypos", overlay.rect.y);
        if overlay.rect.width > 0 {
            comp_pad.set_property("width", overlay.rect.width as i32);
        }
        if overlay.rect.height > 0 {
            comp_pad.set_property("height", overlay.rect.height as i32);
        }
        comp_pad.set_property("alpha", overlay.alpha);
        comp_pad.set_property("zorder", overlay.z_order as u32);

        // 6. Link imagefreeze src -> compositor sink pad
        let freeze_src = imagefreeze.static_pad("src").ok_or("imagefreeze missing src")?;
        freeze_src
            .link(&comp_pad)
            .map_err(|e| format!("link imagefreeze->compositor: {e:?}"))?;

        // 7. Sync state with pipeline
        for elem in [&filesrc, &decodebin, &videoconvert, &videoscale, &imagefreeze] {
            elem.sync_state_with_parent()
                .map_err(|e| format!("sync_state: {e}"))?;
        }

        Ok(vec![src_name, convert_name, scale_name, freeze_name])
    }

    /// Remove an overlay's elements from the running pipeline.
    pub fn remove_overlay(&mut self, overlay_id: &str) -> Result<(), String> {
        let live = self.live_pipeline.as_mut().ok_or("mixer not running")?;
        let pipeline = &live.pipeline;

        let element_names = [
            format!("overlay-src-{overlay_id}"),
            format!("overlay-convert-{overlay_id}"),
            format!("overlay-scale-{overlay_id}"),
            format!("overlay-freeze-{overlay_id}"),
        ];

        for name in &element_names {
            if let Some(elem) = pipeline.by_name(name) {
                let _ = elem.set_state(gst::State::Null);
                let _ = pipeline.remove(&elem);
            }
        }

        Ok(())
    }

    /// Update position / alpha / z-order of an existing overlay.
    pub fn update_overlay_props(
        &self,
        overlay: &OverlayConfig,
    ) -> Result<(), String> {
        let live = self.live_pipeline.as_ref().ok_or("mixer not running")?;
        let compositor = live.video_mixer.as_ref().ok_or("no video compositor")?;

        // Find the pad by iterating sink pads and matching the linked
        // element name.
        let freeze_name = format!("overlay-freeze-{}", overlay.id);
        let freeze = live
            .pipeline
            .by_name(&freeze_name)
            .ok_or_else(|| format!("overlay element {freeze_name} not found"))?;
        let freeze_src = freeze.static_pad("src").ok_or("no src pad")?;
        let comp_pad = freeze_src.peer().ok_or("imagefreeze not linked")?;

        comp_pad.set_property("xpos", overlay.rect.x);
        comp_pad.set_property("ypos", overlay.rect.y);
        comp_pad.set_property("alpha", overlay.alpha);
        comp_pad.set_property("zorder", overlay.z_order as u32);

        Ok(())
    }
}
```

## 3. Wire overlay callbacks to Bridge

Add new Bridge callbacks for overlay operations:

```slint
// bridge.slint (additions)

export struct OverlayItem {
    id:        string,
    slot-id:   string,
    visible:   bool,
    source:    string,   // file path or URL
    x:         int,
    y:         int,
    width:     int,
    height:    int,
    alpha:     float,
    z-order:   int,
}

// In the Bridge global:
in property <[OverlayItem]> overlays: [];
callback add-overlay(OverlayItem);
callback remove-overlay(string);  // (overlay-id)
callback update-overlay(OverlayItem);
```

Rust wiring:

```rust
bridge.on_add_overlay(move |item| {
    let config = overlay_item_to_config(&item);
    overlay_manager.upsert(config.clone());
    // Also add to the live mixer pipeline
    if let Some(mixer) = get_active_mixer() {
        let _ = mixer.add_overlay(&config);
    }
});

bridge.on_update_overlay(move |item| {
    let config = overlay_item_to_config(&item);
    overlay_manager.upsert(config.clone());
    if let Some(mixer) = get_active_mixer() {
        let _ = mixer.update_overlay_props(&config);
    }
});

bridge.on_remove_overlay(move |id| {
    let id = id.to_string();
    overlay_manager.remove(&id);
    if let Some(mixer) = get_active_mixer() {
        let _ = mixer.remove_overlay(&id);
    }
});
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Add `add_overlay`, `remove_overlay`, `update_overlay_props` to `MixerNode` | `mixer.rs` |
| 2 | Add `OverlayItem` struct + callbacks to `bridge.slint` | existing file |
| 3 | Wire Bridge callbacks to `OverlayManager` + `MixerNode` in Rust | `lifecycle.rs` or new |
| 4 | Test with a simple PNG overlay on a running mixer | manual |

---

## Notes

* The `decodebin` approach works with PNG, JPEG, BMP, and other formats
  without explicit format handling.
* `imagefreeze` converts a single decoded frame into a continuous video
  stream, which is required for the `compositor` to mix it with the live
  SRT feeds.
* For animated overlays (GIF, APNG), replace `imagefreeze` with a
  `decodebin` that emits continuous frames.  This is a future extension.
* Performance: each overlay adds one compositor pad.  For <10 overlays
  per source the overhead is negligible.
