# 06 — Slint UI integration

Adds an explicit Start/Stop surface for the migration runtime service
to the Media Backend panel, mirroring the existing gst-pop service
section
(`ui/pages/media_backend_page.slint:131-187`).

This step is **optional** — the service can be driven entirely from
Rust (step 5) and from `adb am start-foreground-service`. But mirroring
the gst-pop UX makes the new surface immediately discoverable for the
user.

## 6.1 New properties + callbacks in `ui/bridge.slint`

Add immediately after the existing gst-pop service block at
`ui/bridge.slint:348-353`:

```diff
     // NEW — explicit service control (gst-pop only).
     callback start-gstpop-service();
     callback stop-gstpop-service();
     // Driven by Rust 1Hz poller while the Media Backend panel is visible.
     in-out property <string> gstpop-service-state: "stopped"; // "stopped"|"starting"|"running"|"error"
     in-out property <bool>   gstpop-service-externally-owned: false;

+    // ── Migration runtime service control (mirrors gst-pop) ─────────────
+    callback start-migration-runtime-service();
+    callback stop-migration-runtime-service();
+    // Driven by Rust 1Hz poller while the Media Backend panel is visible.
+    in-out property <string> migration-runtime-service-state: "stopped"; // "stopped"|"starting"|"running"|"error"

     // ── Mixer screen (MVP-PHASE-11) ──────────────────────────────────────
```

Note the absence of an `externally-owned` flag — the migration runtime
is always in-process (see [00 row 10](./00-plan-review.md)).

## 6.2 Mirror in `ui/state/media_backend.slint`

Add to the `MediaBackend` global (line 16, immediately after the
existing gst-pop service properties):

```diff
     // gst-pop service control
     in-out property <string>           gstpop-service-state: "stopped";
     in-out property <bool>             gstpop-service-externally-owned: false;
+    // Migration runtime service control
+    in-out property <string>           migration-runtime-service-state: "stopped";

     // ── Commands (Slint → Rust) ──────────────────────────────────────
     callback save-settings();
     callback probe();
     callback apply();
     callback start-gstpop-service();
     callback stop-gstpop-service();
+    callback start-migration-runtime-service();
+    callback stop-migration-runtime-service();
     callback start-migration-server(string);
     callback run-migration-test(string);
     callback stop-migration-server();
 }
```

## 6.3 New service section in `ui/pages/media_backend_page.slint`

Add immediately after the existing gst-pop `SERVICE` section (which
ends at line 187). The new section is conditionally visible when
`MediaBackendKind.migration` is selected, mirroring the gst-pop
section's `visible:` predicate at line 133:

```slint
SettingsSection {
    title: @tr("SERVICE");
    visible: Bridge.media-backend == MediaBackendKind.migration;

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
                        Bridge.migration-runtime-service-state == "running"  ? Theme.success :
                        Bridge.migration-runtime-service-state == "starting" ? Theme.warning :
                        Bridge.migration-runtime-service-state == "error"    ? Theme.error-fg :
                        Theme.text-disabled;
                    y: (parent.height - self.height) / 2;
                }
                Text {
                    text:
                        Bridge.migration-runtime-service-state == "running"  ? @tr("Migration runtime running") :
                        Bridge.migration-runtime-service-state == "starting" ? @tr("Migration runtime starting\u{2026}") :
                        Bridge.migration-runtime-service-state == "error"    ? @tr("Migration runtime failed") :
                                                                                @tr("Migration runtime stopped");
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
                    enabled: Bridge.migration-runtime-service-state == "stopped"
                          || Bridge.migration-runtime-service-state == "error";
                    clicked => { Bridge.start-migration-runtime-service(); }
                }
                DestructiveButton {
                    label: @tr("Stop service");
                    enabled: Bridge.migration-runtime-service-state == "running"
                          || Bridge.migration-runtime-service-state == "starting";
                    clicked => { Bridge.stop-migration-runtime-service(); }
                }
            }
        }
    }
}
```

Both the gst-pop and migration-runtime sections share the same `title:
@tr("SERVICE")`. That's intentional — only one is visible at a time
because each section's `visible:` predicate keys off
`Bridge.media-backend`. The user only sees the service block for the
currently selected backend.

> **Slint best-practices reminder.** The repo's pre-commit hooks
> forbid raw hex colors and hard-coded `font-size: Npx` literals in
> `ui/`. The snippet above uses `Theme.*` tokens everywhere
> (`Theme.success`, `Theme.warning`, `Theme.error-fg`,
> `Theme.text-primary`, `Theme.text-disabled`, `Theme.font-size-body`,
> `Theme.surface-card`, `Theme.radius-card`, `Theme.padding-screen`).
> If the design system needs a new token, add it to `ui/theme.slint`
> in the same PR rather than inlining a literal.

## 6.4 Rust callback wiring (`src/backend/lifecycle.rs`)

Insert after the existing gst-pop start/stop callbacks at
`src/backend/lifecycle.rs:79-98`:

```rust
// ── Migration runtime service start / stop ──────────────────────────────
let start_weak = ui.as_weak();
bridge.on_start_migration_runtime_service(move || {
    let weak = start_weak.clone();
    tokio::spawn(async move {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.global::<crate::Bridge>()
                .set_migration_runtime_service_state("starting".into());
        });
        if let Err(err) = crate::migration::service::request_service_start() {
            tracing::error!(?err, "request_service_start (migration runtime)");
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.global::<crate::Bridge>()
                    .set_migration_runtime_service_state("error".into());
            });
        }
    });
});

let stop_weak = ui.as_weak();
bridge.on_stop_migration_runtime_service(move || {
    crate::migration::service::request_service_stop();
    let weak = stop_weak.clone();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.global::<crate::Bridge>()
            .set_migration_runtime_service_state("stopping".into());
    });
});
```

Notes:

* The `"starting"` and `"stopping"` states are pushed eagerly so the
  UI buttons disable themselves immediately. The 1 Hz poller (next
  section) then converges the state to `"running"` / `"stopped"`
  / `"error"` based on actual runtime liveness.
* `tokio::spawn` is used (rather than calling
  `request_service_start` directly) so the JNI attach inside the
  helper doesn't block the Slint event loop.

## 6.5 1 Hz status poller

Insert after the existing gst-pop poller at
`src/backend/lifecycle.rs:100-124`:

```rust
// ── Migration runtime: 1Hz status poller ──────────────────────────────────
let poll_weak = ui.as_weak();
tokio::spawn(async move {
    let mut ticker =
        tokio::time::interval(std::time::Duration::from_millis(1000));
    loop {
        ticker.tick().await;
        let state_str: &'static str = match crate::migration::service::query_status() {
            Ok(json) => {
                if json.contains("\"running\"") {
                    "running"
                } else if json.contains("\"error\"") {
                    "error"
                } else {
                    "stopped"
                }
            }
            Err(_) => "stopped",
        };
        let _ = poll_weak.upgrade_in_event_loop(move |ui| {
            let b = ui.global::<crate::Bridge>();
            if b.get_active_panel() != crate::Panel::MediaBackend {
                return;
            }
            b.set_migration_runtime_service_state(state_str.into());
        });
    }
});
```

The poller only writes to the Bridge while the Media Backend panel is
visible. This is the same anti-CPU-burn behaviour the gst-pop poller
implements at `lifecycle.rs:117-119`.

A small step up for future work: replace the `contains` checks with a
proper `serde_json::from_str::<MigrationRuntimeStatus>(...)` once the
schema in [01-rust-jni-bridge.md §1.2](./01-rust-jni-bridge.md#12-status-json-shape)
is reified into a Rust struct shared between `lib.rs` and `lifecycle.rs`.

## 6.6 Pre-commit hook reminder

Two of the four hooks in `.pre-commit-config.yaml` enforce Slint style:

* `forbid-raw-hex-colors` — disallows `#aabbcc` / `#aabbccdd` in
  `ui/components/*.slint`, `ui/pages/*.slint`, `ui/main.slint`,
  `ui/bridge.slint`.
* `forbid-hard-coded-font-size` — disallows `font-size: Npx` in
  `ui/components/*.slint` and `ui/pages/*.slint`.

The snippet in §6.3 above is hook-clean. If you reach for a new token,
add it to `ui/theme.slint` in the same PR.

## 6.7 i18n note

All user-visible strings use `@tr(...)`. When the implementation PR
lands, add corresponding entries to whichever i18n files the repo
uses (`ui/i18n/...`); the four new strings are:

* `Migration runtime running`
* `Migration runtime starting…` (use `\u{2026}` in the Slint source so the
  raw `…` doesn't sneak into source files)
* `Migration runtime failed`
* `Migration runtime stopped`
