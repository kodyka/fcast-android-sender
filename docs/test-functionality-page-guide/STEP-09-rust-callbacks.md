# STEP 09 — Wire Rust Callbacks

**File:** `src/lib.rs` (or new `src/test_functionality.rs`)

---

## Goal

Implement the three Rust callbacks that the Slint page triggers:

1. `start-test()` → build and start the GStreamer test pipeline
2. `stop-test()` → tear down the pipeline
3. `pick-test-overlay-image()` → open Android intent / desktop file
   dialog

---

## 1. Callback registration

```rust
// src/lib.rs — inside the UI init function (after MainWindow::new())
// or in a new src/test_functionality.rs module

use slint::{ComponentHandle, Weak};

/// Wire test-functionality callbacks on the Bridge global.
pub fn wire_test_functionality(ui: &MainWindow) {
    let weak = ui.as_weak();
    wire_start_test(weak.clone());
    wire_stop_test(weak.clone());
    wire_pick_overlay_image(weak);
}
```

---

## 2. `on_start_test` — build the GStreamer pipeline

```rust
fn wire_start_test(weak: Weak<MainWindow>) {
    let ui = weak.upgrade().unwrap();
    ui.global::<Bridge>().on_start_test({
        let weak = weak.clone();
        move || {
            let weak = weak.clone();

            // Update state to "starting"
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.global::<Bridge>().set_test_state(MixerState::Starting);
            });

            tokio::spawn(async move {
                // ── Step A: Read configuration from Bridge ────────────
                // (done inside upgrade_in_event_loop, copy values to local vars)
                //
                // let camera_idx = ui.global::<Bridge>().get_test_camera_idx();
                // let resolution_idx = ui.global::<Bridge>().get_test_resolution_idx();
                // let framerate_idx = ui.global::<Bridge>().get_test_framerate_idx();
                // let srt_source = ui.global::<Bridge>().get_test_srt_source();
                // let overlay_path = ui.global::<Bridge>().get_test_overlay_image_path();
                // let overlay_enabled = ui.global::<Bridge>().get_test_overlay_enabled();
                // ... etc.

                // ── Step B: Build GStreamer pipeline ──────────────────
                // The pipeline structure depends on which sources are enabled.
                //
                // Minimal pipeline (camera only):
                //   camerasrc ! videoconvert ! videoscale ! autovideosink
                //
                // With SRT source:
                //   srtsrc uri=... ! tsdemux ! h264parse ! decodebin ! queue
                //     → compositor pad 1
                //   camerasrc ! videoconvert
                //     → compositor pad 0
                //   compositor ! autovideosink
                //
                // With image overlay:
                //   [...camera/srt...] ! compositor
                //   filesrc location=overlay.png ! decodebin ! imagefreeze
                //     → compositor pad N (zorder=10, alpha=0.8, xpos=..., ypos=...)
                //
                // Android-specific: use `ahcsrc` (Android Hardware Camera Source)
                // or `amcvideodec` for decoding.
                //
                // Pattern reference: src/migration/nodes/mixer.rs
                //   - See how SRT sources are added as compositor pads
                //   - See how pad properties (alpha, zorder, xpos, ypos) are set

                // ── Step C: Start the pipeline ───────────────────────
                // pipeline.set_state(gst::State::Playing)?;

                // ── Step D: Update UI state ──────────────────────────
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<Bridge>().set_test_state(MixerState::Running);
                });
            });
        }
    });
}
```

### GStreamer compositor pad configuration

```rust
// Setting overlay pad properties on the GStreamer compositor:
//
// let overlay_pad = compositor.request_pad_simple("sink_%u").unwrap();
// overlay_pad.set_property("xpos",   overlay_x as i32);
// overlay_pad.set_property("ypos",   overlay_y as i32);
// overlay_pad.set_property("width",  overlay_width as i32);
// overlay_pad.set_property("height", overlay_height as i32);
// overlay_pad.set_property("alpha",  overlay_alpha as f64);
// overlay_pad.set_property("zorder", overlay_z_order as u32);
```

---

## 3. `on_stop_test` — tear down the pipeline

```rust
fn wire_stop_test(weak: Weak<MainWindow>) {
    let ui = weak.upgrade().unwrap();
    ui.global::<Bridge>().on_stop_test({
        let weak = weak.clone();
        move || {
            let weak = weak.clone();

            // Update state to "stopping"
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.global::<Bridge>().set_test_state(MixerState::Stopping);
            });

            tokio::spawn(async move {
                // ── Step A: Stop the pipeline ────────────────────────
                // pipeline.set_state(gst::State::Null)?;

                // ── Step B: Drop resources ───────────────────────────
                // Drop the pipeline, release camera, close SRT socket.

                // ── Step C: Update UI state ──────────────────────────
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<Bridge>().set_test_state(MixerState::Idle);
                    ui.global::<Bridge>().set_test_error_text("".into());
                });
            });
        }
    });
}
```

---

## 4. `on_pick_test_overlay_image` — file picker

```rust
fn wire_pick_overlay_image(weak: Weak<MainWindow>) {
    let ui = weak.upgrade().unwrap();
    ui.global::<Bridge>().on_pick_test_overlay_image({
        let weak = weak.clone();
        move || {
            let weak = weak.clone();

            #[cfg(target_os = "android")]
            {
                // Use Android Intent API via JNI:
                //
                // Java side (in app/src/main/java/.../MainActivity.java):
                //   private static final int REQUEST_PICK_IMAGE = 42;
                //
                //   public void pickImageForOverlay() {
                //       Intent intent = new Intent(Intent.ACTION_GET_CONTENT);
                //       intent.setType("image/*");
                //       startActivityForResult(intent, REQUEST_PICK_IMAGE);
                //   }
                //
                //   @Override
                //   protected void onActivityResult(int request, int result, Intent data) {
                //       if (request == REQUEST_PICK_IMAGE && result == RESULT_OK) {
                //           Uri uri = data.getData();
                //           // Copy URI content to app-private file:
                //           String localPath = copyUriToFile(uri, "test_overlay.png");
                //           // Call Rust via JNI:
                //           nativeSetOverlayImagePath(localPath);
                //       }
                //   }
                //
                // Rust JNI side:
                //   #[no_mangle]
                //   pub extern "C" fn Java_...nativeSetOverlayImagePath(
                //       env: JNIEnv, _: JClass, path: JString
                //   ) {
                //       let path: String = env.get_string(path).unwrap().into();
                //       // Use stored Weak<MainWindow> to update Bridge:
                //       let _ = WEAK.upgrade_in_event_loop(move |ui| {
                //           ui.global::<Bridge>()
                //             .set_test_overlay_image_path(path.into());
                //       });
                //   }
                //
                // Trigger the Java method from Rust:
                //   let activity = crate::android_context().unwrap();
                //   let mut env = activity.vm.attach_current_thread().unwrap();
                //   env.call_method(
                //       activity.context.as_obj(),
                //       "pickImageForOverlay", "()V", &[]
                //   ).unwrap();

                log::info!("Launching Android image picker intent");
            }

            #[cfg(not(target_os = "android"))]
            {
                // Desktop: use rfd (Rust File Dialog) crate
                //
                // Cargo.toml dependency:
                //   [dependencies]
                //   rfd = "0.14"
                //
                // Implementation:
                //   std::thread::spawn(move || {
                //       let path = rfd::FileDialog::new()
                //           .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "webp"])
                //           .set_title("Select overlay image")
                //           .pick_file();
                //
                //       if let Some(p) = path {
                //           let _ = weak.upgrade_in_event_loop(move |ui| {
                //               ui.global::<Bridge>()
                //                   .set_test_overlay_image_path(
                //                       p.display().to_string().into()
                //                   );
                //           });
                //       }
                //   });

                log::info!("Launching desktop file picker");
            }
        }
    });
}
```

---

## 5. Error handling

```rust
// When the GStreamer pipeline encounters an error, push it to the UI:
//
// let bus = pipeline.bus().unwrap();
// bus.add_watch(move |_, msg| {
//     match msg.view() {
//         gst::MessageView::Error(err) => {
//             let error_text = format!(
//                 "{}: {}",
//                 err.error(),
//                 err.debug().unwrap_or_default()
//             );
//             let _ = weak.upgrade_in_event_loop(move |ui| {
//                 ui.global::<Bridge>().set_test_error_text(error_text.into());
//                 ui.global::<Bridge>().set_test_state(MixerState::Error);
//             });
//         }
//         gst::MessageView::Eos(..) => {
//             let _ = weak.upgrade_in_event_loop(move |ui| {
//                 ui.global::<Bridge>().set_test_state(MixerState::Idle);
//             });
//         }
//         _ => {}
//     }
//     glib::ControlFlow::Continue
// });
```

---

## Wire-up checklist

| # | Action | File |
|---|--------|------|
| 1 | Create `wire_test_functionality()` function | `src/test_functionality.rs` (new) or `src/lib.rs` |
| 2 | Call `wire_test_functionality(&ui)` during UI init | `src/lib.rs` |
| 3 | Implement GStreamer pipeline building (reuse patterns from `src/migration/nodes/mixer.rs`) | `src/test_functionality.rs` |
| 4 | Implement Android image picker intent + JNI callback | `app/src/main/java/.../MainActivity.java` + Rust JNI |
| 5 | (Desktop) Add `rfd` to `Cargo.toml` if needed | `Cargo.toml` |
| 6 | Add bus watch for error handling | `src/test_functionality.rs` |

---

## Notes

* The `upgrade_in_event_loop` pattern is the standard way to update Slint
  properties from async Rust code.  It ensures the update happens on the
  UI thread.
* The pipeline state transitions follow the same pattern as the mixer:
  `idle → starting → running → stopping → idle`, with `error` as a
  possible state from any transition.
* The Android image picker requires a Java bridge method.  The pattern
  is already used in the project for other intent-based operations.
