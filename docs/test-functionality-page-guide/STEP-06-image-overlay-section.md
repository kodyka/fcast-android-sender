# STEP 06 — Image Overlay Section

**Internal component for:** `ui/pages/test_functionality_page.slint`

---

## Goal

Build the `ImageOverlayCard` component, adapted from the Moblin
`WidgetImageSettingsView.swift` (lines 1-70) and
`WidgetImagePickerView` patterns.

---

## Design reference

### From `WidgetImageSettingsView.swift`

```swift
// Moblin original (simplified):
struct WidgetImagePickerView: View {
    @Binding var image: UIImage?
    @State private var selectedImageItem: PhotosPickerItem?

    var body: some View {
        Section {
            PhotosPicker(selection: $selectedImageItem, matching: .images) {
                if let image {
                    Image(uiImage: image)              // <-- show preview
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .frame(width: 1920/6, height: 1080/6)
                } else {
                    Text("Select image")               // <-- placeholder
                }
            }
        }
    }
}
```

### Slint adaptation strategy

| Moblin element | Challenge in Slint | Our solution |
|----------------|--------------------|--------------|
| `PhotosPicker` | Slint has no native photo/file picker | `LineEdit` (path) + `PrimaryButton` ("Browse") — Rust opens Android intent or desktop dialog |
| `Image(uiImage:)` preview | Slint `Image` needs compile-time `@image-url` or Rust `NativeImage` | Placeholder `Rectangle` with status text; real preview when Rust pushes a texture (future) |
| Image effects (alpha, position) via `WidgetEffectsView` | Need sliders for position, size, alpha, z-order | `SettingsSliderRow` controls with ranges matching GStreamer compositor pad limits |

---

## Component snippet

```slint
// test_functionality_page.slint — Internal image overlay card component.
//
// Adapted from:
//   draft/moblin-ui/.../WidgetImageSettingsView.swift
//   draft/moblin-ui/.../WidgetImagePickerView (PhotosPicker pattern)
//
// Since Slint has no native photo/file picker, we use a text-field for
// the path and a "Browse" button that triggers a Rust-side file dialog.

component ImageOverlayCard inherits Rectangle {
    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 460px;

    VerticalLayout {
        padding-left: Theme.padding-screen;
        padding-right: Theme.padding-screen;
        padding-top: Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing: Theme.spacing-default;

        // ── Header: title + enable toggle ─────────────────────────────
        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: @tr("Image Overlay");
                color: Theme.text-primary;
                font-size: Theme.font-size-heading;
                vertical-alignment: center;
                horizontal-stretch: 1;
            }
            Switch {
                checked <=> Bridge.test-overlay-enabled;
            }
        }

        // ── Image selection ───────────────────────────────────────────
        // Replaces WidgetImagePickerView's PhotosPicker.
        //
        // The path field shows the file selected by the Rust-side
        // picker.  The user can also type/paste a path manually.
        Text {
            text: @tr("Image file path");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        HorizontalLayout {
            spacing: Theme.spacing-default;
            LineEdit {
                placeholder-text: @tr("/sdcard/DCIM/overlay.png");
                text <=> Bridge.test-overlay-image-path;
                horizontal-stretch: 1;
            }
            PrimaryButton {
                label: @tr("Browse");
                clicked => {
                    Bridge.pick-test-overlay-image();
                }
            }
        }

        // ── Image preview placeholder ─────────────────────────────────
        // Maps to WidgetImagePickerView's conditional image display:
        //   if let image { Image(...) } else { Text("Select image") }
        //
        // Real preview requires Rust to push a NativeImage texture from
        // the loaded file.  For now, show a status placeholder.
        Rectangle {
            height: 120px;
            border-radius: Theme.radius-card;
            background: Bridge.test-overlay-image-path != ""
                ? Theme.surface-bar
                : Theme.surface-primary;

            Text {
                text: Bridge.test-overlay-image-path != ""
                    ? @tr("Preview available when test is running")
                    : @tr("No image selected");
                color: Theme.text-secondary;
                font-size: Theme.font-size-label;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        // ── Position controls ─────────────────────────────────────────
        // These map to the scene editor's widget positioning in Moblin.
        // In WidgetImageSettingsView.swift, position is set via drag
        // in the scene preview.  We use sliders instead.
        SettingsSliderRow {
            title: @tr("X position");
            unit: @tr(" px");
            minimum: -1920;
            maximum: 3840;
            show-fractional: false;
            value <=> Bridge.test-overlay-x;
        }

        SettingsSliderRow {
            title: @tr("Y position");
            unit: @tr(" px");
            minimum: -1080;
            maximum: 2160;
            show-fractional: false;
            value <=> Bridge.test-overlay-y;
        }

        // ── Size controls ─────────────────────────────────────────────
        // Maps to WidgetImagePickerView's frame(width:height:).
        // 0 means "use original image dimensions".
        SettingsSliderRow {
            title: @tr("Width");
            unit: @tr(" px");
            minimum: 0;
            maximum: 1920;
            show-fractional: false;
            value <=> Bridge.test-overlay-width;
        }

        SettingsSliderRow {
            title: @tr("Height");
            unit: @tr(" px");
            minimum: 0;
            maximum: 1080;
            show-fractional: false;
            value <=> Bridge.test-overlay-height;
        }

        // ── Compositing controls ──────────────────────────────────────
        // Alpha: maps to WidgetEffectsView opacity control.
        SettingsSliderRow {
            title: @tr("Alpha");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> Bridge.test-overlay-alpha;
        }

        // Z-order: GStreamer compositor pad property.
        // Higher values render on top. int ↔ float conversion needed.
        SettingsSliderRow {
            title: @tr("Z-order");
            minimum: 0;
            maximum: 99;
            show-fractional: false;
            value: Bridge.test-overlay-z-order;
            changed(v) => {
                Bridge.test-overlay-z-order = v;
            }
        }
    }
}
```

---

## Usage in the page

```slint
// Inside the ScrollView VerticalLayout:
SettingsSection {
    title: @tr("IMAGE OVERLAY");

    ImageOverlayCard { }
}
```

---

## Rust-side image picker implementation

```rust
// Called when Bridge.pick-test-overlay-image() fires:

#[cfg(target_os = "android")]
fn pick_image_android(weak: slint::Weak<MainWindow>) {
    // 1. Get the Android activity context
    let ctx = crate::android_context().expect("android_context");
    let mut env = ctx.vm.attach_current_thread().expect("JNI attach");

    // 2. Launch an ACTION_GET_CONTENT intent for images
    //    Intent intent = new Intent(Intent.ACTION_GET_CONTENT);
    //    intent.setType("image/*");
    //    activity.startActivityForResult(intent, REQUEST_PICK_IMAGE);
    //
    // 3. In the onActivityResult handler (Java bridge):
    //    - Copy the URI content to a local file in app's files dir
    //    - Call back into Rust with the local file path
    //
    // 4. Set the Bridge property:
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.global::<Bridge>()
            .set_test_overlay_image_path("/data/.../picked_image.png".into());
    });
}

#[cfg(not(target_os = "android"))]
fn pick_image_desktop(weak: slint::Weak<MainWindow>) {
    // Desktop: use rfd (Rust File Dialog) or native-dialog
    //   let path = rfd::FileDialog::new()
    //       .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "webp"])
    //       .pick_file();
    //
    //   if let Some(p) = path {
    //       let _ = weak.upgrade_in_event_loop(move |ui| {
    //           ui.global::<Bridge>()
    //               .set_test_overlay_image_path(p.display().to_string().into());
    //       });
    //   }
}
```

---

## Wire-up checklist

| # | Action |
|---|--------|
| 1 | Add `ImageOverlayCard` component to `test_functionality_page.slint` |
| 2 | Use inside a `SettingsSection { title: @tr("IMAGE OVERLAY"); }` block |
| 3 | Ensure all `Bridge.test-overlay-*` properties exist (STEP 02) |
| 4 | Ensure `Bridge.pick-test-overlay-image()` callback exists (STEP 02) |
| 5 | Implement Rust-side file picker (Android intent + desktop dialog) |

---

## Notes

* The `value <=> Bridge.test-overlay-x` two-way binding works because
  both `SettingsSliderRow.value` and `Bridge.test-overlay-x` are `float`.
* The z-order slider uses one-way bind + explicit write-back because
  `Bridge.test-overlay-z-order` is `int` while `SettingsSliderRow.value`
  is `float`.  This is the same pattern used in `MixerSlotControls`
  (mixer_page.slint line 142).
* For the future: to show a real image preview, Rust can load the image
  file with the `image` crate, convert to RGBA, and push it as a
  `slint::Image` via `slint::Image::from_rgba8()`.
