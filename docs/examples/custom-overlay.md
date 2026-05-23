# Example: Custom Overlay Implementation

## Adding a text overlay (clock / timer)

Instead of a static image, create a dynamic text overlay using GStreamer's `textoverlay` element:

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

The mixer integration (`src/migration/nodes/mixer.rs`) would handle `DynamicText` by creating a `textoverlay` element instead of `imagefreeze`:

```rust
OverlaySource::DynamicText { template, font } => {
    let textoverlay = Self::make_element("textoverlay", None)?;
    textoverlay.set_property("text", &template);
    textoverlay.set_property("font-desc", &font);
    // ... connect to compositor pad ...
}
```
