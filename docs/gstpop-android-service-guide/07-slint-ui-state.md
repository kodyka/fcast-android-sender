# 7 · Slint UI state additions

The Slint layer needs three things: a new state variant (`Starting`),
two new callbacks for explicit Start/Stop in the Media Backend page,
and a status pill that renders the new state.

## 7.1 `ui/bridge.slint` — state enum

Edit `ui/bridge.slint:55-60`:

```diff
 export enum MediaBackendState {
     disconnected,
+    starting,    // NEW — service-start requested, daemon not yet listening
     probing,
     ready,
     error,
 }
```

## 7.2 `ui/bridge.slint` — new callbacks

Edit `ui/bridge.slint:316-320` (the Media-backend callbacks block):

```diff
     // ── Media backend selector callbacks (MVP-PHASE-12) ────────────────
     callback save-media-backend-settings();
     callback probe-media-backend();
     callback apply-media-backend();
+
+    // NEW — explicit service control (gst-pop only).
+    callback start-gstpop-service();
+    callback stop-gstpop-service();
+    // Triggered by Rust 1Hz poller while the panel is visible.
+    in-out property <string> gstpop-service-state: "stopped"; // "stopped"|"starting"|"running"|"error"
+    in-out property <bool>   gstpop-service-externally-owned: false;
```

`gstpop-service-state` is a plain string (rather than a Slint enum)
so the Rust poller can write the JSON `state` field through verbatim
without a translation function on every tick.

## 7.3 `ui/pages/media_backend_page.slint` — status pill

Edit the status pill block (`ui/pages/media_backend_page.slint:40-89`)
to handle the new state.

```diff
                 Rectangle {
                     background: Bridge.media-backend-state == MediaBackendState.error
                         ? Theme.error.darker(35%)
                         : Theme.surface-card;
                     border-radius: Theme.radius-card;
                     min-height: 56px;
                     HorizontalLayout {
                         padding: Theme.padding-screen;
                         spacing: 8px;
                         Rectangle {
                             width: 12px;
                             height: 12px;
                             border-radius: 6px;
                             background:
                                 Bridge.media-backend-state == MediaBackendState.ready    ? Theme.success :
+                                Bridge.media-backend-state == MediaBackendState.starting ? Theme.warning :
                                 Bridge.media-backend-state == MediaBackendState.probing  ? Theme.warning :
                                 Bridge.media-backend-state == MediaBackendState.error    ? Theme.error-fg :
                                 Theme.text-disabled;
                             y: (parent.height - self.height) / 2;
                         }
                         VerticalLayout {
                             spacing: 2px;
                             horizontal-stretch: 1;
                             Text {
                                 text:
                                     Bridge.media-backend-state == MediaBackendState.ready    ? @tr("Ready") :
+                                    Bridge.media-backend-state == MediaBackendState.starting ? @tr("Starting gst-pop service…") :
                                     Bridge.media-backend-state == MediaBackendState.probing  ? @tr("Probing…") :
                                     Bridge.media-backend-state == MediaBackendState.error    ? @tr("Error") :
                                     @tr("Disconnected");
                                 color: Theme.text-primary;
                                 font-size: Theme.font-size-body;
                             }
                             if Bridge.media-backend-status-text != "": Text {
                                 text: Bridge.media-backend-status-text;
                                 color: Theme.text-secondary;
                                 font-size: Theme.font-size-label;
                                 wrap: word-wrap;
                             }
                             if Bridge.media-backend-error-text != "": Text {
                                 text: Bridge.media-backend-error-text;
                                 color: Theme.error-fg;
                                 font-size: Theme.font-size-label;
                                 wrap: word-wrap;
                             }
                         }
                     }
                 }
```

## 7.4 `ui/pages/media_backend_page.slint` — new "Service" section

Add this block below the existing `Apply` button row. It surfaces
Start/Stop independently of Apply, and shows the underlying daemon
state from the poller.

```slint
SettingsSection {
    title: @tr("SERVICE");
    visible: Bridge.media-backend == MediaBackendKind.gst-pop;

    Rectangle {
        background: Theme.surface-card;
        border-radius: Theme.radius-card;
        min-height: 96px;
        VerticalLayout {
            padding: Theme.padding-screen;
            spacing: 12px;

            HorizontalLayout {
                spacing: 8px;
                Rectangle {
                    width: 12px;
                    height: 12px;
                    border-radius: 6px;
                    background:
                        Bridge.gstpop-service-state == "running"  ? Theme.success :
                        Bridge.gstpop-service-state == "starting" ? Theme.warning :
                        Bridge.gstpop-service-state == "error"    ? Theme.error-fg :
                        Theme.text-disabled;
                    y: (parent.height - self.height) / 2;
                }
                Text {
                    text:
                        Bridge.gstpop-service-externally-owned    ? @tr("Using external gst-pop daemon") :
                        Bridge.gstpop-service-state == "running"  ? @tr("gst-pop service running") :
                        Bridge.gstpop-service-state == "starting" ? @tr("gst-pop service starting…") :
                        Bridge.gstpop-service-state == "error"    ? @tr("gst-pop service failed") :
                                                                    @tr("gst-pop service stopped");
                    color: Theme.text-primary;
                    font-size: Theme.font-size-body;
                    horizontal-stretch: 1;
                    vertical-alignment: center;
                }
            }

            HorizontalLayout {
                spacing: 12px;
                PrimaryButton {
                    label: @tr("Start service");
                    enabled: Bridge.gstpop-service-state == "stopped"
                          || Bridge.gstpop-service-state == "error";
                    clicked => { Bridge.start-gstpop-service(); }
                }
                DestructiveButton {
                    label: @tr("Stop service");
                    enabled: Bridge.gstpop-service-state == "running"
                          || Bridge.gstpop-service-state == "starting";
                    clicked => { Bridge.stop-gstpop-service(); }
                }
            }
        }
    }
}
```

The buttons are deliberately separate from the `Apply` button. Apply
saves config + triggers a probe. Start/Stop directly drive the
service for cases where the user wants to keep the gst-pop config but
toggle the daemon (e.g., to free port 9000 temporarily).

## 7.5 Rust callback registration

In `src/backend/lifecycle.rs::register`, alongside the existing
`on_apply_media_backend` / `on_save_media_backend_settings` /
`on_probe_media_backend` blocks:

```rust
// ── Start / Stop service ──────────────────────────────────────────
let start_weak = ui.as_weak();
bridge.on_start_gstpop_service(move || {
    let config = read_config_from_bridge(&start_weak);
    let weak = start_weak.clone();
    tokio::spawn(async move {
        push_state(&weak, crate::MediaBackendState::Starting);
        if let Err(err) = super::gstpop::service::request_service_start(&config) {
            push_error(&weak, &format!("service start failed: {err}"));
        }
        // Poller (7.6) will update the pill from here on.
    });
});

let stop_weak = ui.as_weak();
bridge.on_stop_gstpop_service(move || {
    super::gstpop::service::request_service_stop();
    let weak = stop_weak.clone();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<crate::Bridge>();
        bridge.set_gstpop_service_state("stopping".into());
    });
});
```

## 7.6 Rust 1Hz status poller

The service does not push status to the UI (binder API is `null` per
step 4.1). Instead, the Rust side polls `embedded_status()` while the
Media Backend panel is visible. Put this in `register`:

```rust
let poll_weak = ui.as_weak();
tokio::spawn(async move {
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(1000));
    loop {
        ticker.tick().await;
        let Some(ui) = poll_weak.upgrade() else { return; };
        // Skip when the panel isn't on-screen — Bridge.active-panel is the
        // single source of truth for "which screen is the user on".
        if ui.global::<crate::Bridge>().get_active_panel() != crate::Panel::MediaBackend {
            continue;
        }
        let status = super::gstpop::embedded::embedded_status();
        let state_str = match status.state {
            EmbeddedState::Stopped   => "stopped",
            EmbeddedState::Starting  => "starting",
            EmbeddedState::Running { .. } => "running",
            EmbeddedState::Error     => "error",
        };
        let externally = matches!(
            status.state,
            EmbeddedState::Running { externally_owned: true }
        );
        let _ = poll_weak.upgrade_in_event_loop(move |ui| {
            let b = ui.global::<crate::Bridge>();
            b.set_gstpop_service_state(state_str.into());
            b.set_gstpop_service_externally_owned(externally);
        });
    }
});
```

Note: `crate::Panel::MediaBackend` is the enum variant for the Media
Backend page. Check the actual identifier in `ui/bridge.slint` and
match it exactly — Slint enum variants become Rust enum variants
with the same casing.

The poller is process-lifetime — fine because it self-throttles when
the panel isn't visible and the cost per tick is `RwLock::read` +
one `upgrade_in_event_loop` call.

## 7.7 Status text propagation

When apply/probe paths push status text, include the daemon state for
clarity:

```rust
// src/backend/lifecycle.rs — extend push_status.
fn push_starting(weak: &Weak<MainWindow>, status: &EmbeddedStatus) {
    let text = match status.state {
        EmbeddedState::Stopped  => "gst-pop service stopped".into(),
        EmbeddedState::Starting => "Starting gst-pop service…".into(),
        EmbeddedState::Running { externally_owned: true }  =>
            "Using external gst-pop daemon".into(),
        EmbeddedState::Running { externally_owned: false } =>
            format!("gst-pop running on {}:{}", status.bind, status.port),
        EmbeddedState::Error    =>
            status.last_error.clone().unwrap_or_else(|| "gst-pop failed".into()),
    };
    let weak = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.global::<crate::Bridge>()
            .set_media_backend_status_text(text.into());
    });
}
```

## 7.8 Optional: ProgressIndicator while Starting

If you want a spinner instead of just a static "Starting…" label,
the existing components folder has `Spinner` or similar — drop it
into the status pill conditionally:

```slint
if Bridge.media-backend-state == MediaBackendState.starting: Spinner {
    width: 16px;
    height: 16px;
    y: (parent.height - self.height) / 2;
}
```

Not required for the milestone — the text alone is enough signal.

Next: [08-shutdown-policy.md](./08-shutdown-policy.md).
