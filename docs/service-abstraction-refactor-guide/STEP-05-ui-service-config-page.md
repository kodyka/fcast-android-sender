# STEP 05 — UI Service Configuration Page

**Phase:** 2 (Configuration System)
**New file:** `ui/pages/service_config_page.slint`

---

## Goal

Create a dedicated settings page where the user can toggle each service
on/off, choose the hosting mode, and control auto-start behaviour.

---

## 1. Add new enums and struct to `bridge.slint`

```slint
// bridge.slint — add near the existing enums

export enum ServiceMode {
    embedded,
    android-service,
    external,
}

export struct ServiceConfig {
    gstpop-enabled:            bool,
    migration-runtime-enabled: bool,
    auto-start-services:       bool,
    service-mode:              ServiceMode,
}
```

## 2. Add a `ServiceConfig` global to `ui/state/`

```slint
// ui/state/service_config.slint

import { ServiceMode, ServiceConfig } from "../bridge.slint";

export global ServiceConfigBridge {
    // ── State (Rust -> Slint) ─────────────────────────────────────────
    in property <ServiceConfig> config: {
        gstpop-enabled:            true,
        migration-runtime-enabled: true,
        auto-start-services:       true,
        service-mode:              ServiceMode.embedded,
    };

    // ── Commands (Slint -> Rust) ──────────────────────────────────────
    callback save-config(ServiceConfig);
    callback reset-config();
}
```

Register in `ui/state/index.slint`:

```slint
import { ServiceConfigBridge } from "service_config.slint";
export { ServiceConfigBridge }
```

And re-export from `ui/main.slint`:

```slint
import { ..., ServiceConfigBridge } from "state/index.slint";
export { ..., ServiceConfigBridge }
```

## 3. Create the page component

```slint
// ui/pages/service_config_page.slint

import { Switch, ScrollView, ComboBox } from "std-widgets.slint";
import { ServiceConfigBridge } from "../state/service_config.slint";
import { ServiceMode, ServiceConfig } from "../bridge.slint";
import { PanelBridge } from "../state/panel_bridge.slint";
import { Theme } from "../theme.slint";
import { PrimaryButton, TextButton } from "../components/buttons.slint";
import { SettingsSection } from "../components/settings_rows.slint";
import { PanelHeader, Card, FormRow } from "../components/panel_chrome.slint";

export component ServiceConfigPage inherits Rectangle {
    property <[string]> mode-labels: [
        @tr("Embedded (in-process)"),
        @tr("Android Service"),
        @tr("External daemon"),
    ];

    pure function mode-to-index(m: ServiceMode) -> int {
        if m == ServiceMode.embedded          { return 0; }
        if m == ServiceMode.android-service   { return 1; }
        return 2;
    }
    pure function index-to-mode(i: int) -> ServiceMode {
        if i == 0 { return ServiceMode.embedded;        }
        if i == 1 { return ServiceMode.android-service; }
        return ServiceMode.external;
    }

    width: 100%;
    height: 100%;
    background: Theme.surface-primary;

    VerticalLayout {
        PanelHeader {
            title: @tr("Service Configuration");
            close-clicked => { PanelBridge.pop(); }
        }

        ScrollView {
            VerticalLayout {
                alignment: start;
                spacing: Theme.spacing-default;
                padding: Theme.padding-screen;

                // ── gst-pop toggle ────────────────────────────────
                SettingsSection {
                    title: @tr("GST-POP SERVICE");
                    Card {
                        FormRow {
                            label: @tr("Enable gst-pop");
                            Switch {
                                checked: ServiceConfigBridge.config.gstpop-enabled;
                                toggled => {
                                    // The page keeps a local draft; save on "Apply".
                                }
                            }
                        }
                    }
                }

                // ── Migration toggle ──────────────────────────────
                SettingsSection {
                    title: @tr("MIGRATION RUNTIME");
                    Card {
                        FormRow {
                            label: @tr("Enable migration runtime");
                            Switch {
                                checked: ServiceConfigBridge.config.migration-runtime-enabled;
                                toggled => { }
                            }
                        }
                    }
                }

                // ── Global settings ───────────────────────────────
                SettingsSection {
                    title: @tr("GLOBAL");

                    Card {
                        FormRow {
                            label: @tr("Auto-start services on launch");
                            Switch {
                                checked: ServiceConfigBridge.config.auto-start-services;
                                toggled => { }
                            }
                        }
                    }

                    Card {
                        FormRow {
                            label: @tr("Default service mode");
                            ComboBox {
                                model: root.mode-labels;
                                current-index: mode-to-index(
                                    ServiceConfigBridge.config.service-mode);
                                selected(v) => { }
                            }
                        }
                    }
                }
            }
        }

        // ── Bottom action bar ─────────────────────────────────────────
        Rectangle {
            height: 72px;
            background: Theme.surface-card;
            HorizontalLayout {
                padding: Theme.padding-screen;
                spacing: Theme.spacing-default;
                Rectangle { horizontal-stretch: 1; }
                TextButton {
                    label: @tr("Reset");
                    clicked => { ServiceConfigBridge.reset-config(); }
                }
                PrimaryButton {
                    label: @tr("Apply & Save");
                    clicked => {
                        ServiceConfigBridge.save-config(
                            ServiceConfigBridge.config);
                    }
                }
            }
        }
    }
}
```

## 4. Register the new panel variant

In `bridge.slint`, add a new variant to the `Panel` enum:

```slint
export enum Panel {
    // ... existing variants ...
    service-config,
}
```

In `ui/main.slint`, add the conditional instantiation:

```slint
if PanelBridge.active == Panel.service-config: ServiceConfigPage { }
```

## 5. Rust-side wiring

```rust
// Inside BackendLifecycle::register (or a new ServiceLifecycle struct)

let service_cfg = ui.global::<ServiceConfigBridge>();
service_cfg.on_save_config(move |cfg| {
    // Map Slint ServiceConfig -> Rust ServiceOptions
    let gstpop_opts = ServiceOptions {
        enabled: cfg.gstpop_enabled,
        auto_start: cfg.auto_start_services,
        mode: match cfg.service_mode {
            ServiceMode::Embedded       => crate::service::ServiceMode::Embedded,
            ServiceMode::AndroidService => crate::service::ServiceMode::AndroidService,
            ServiceMode::External       => crate::service::ServiceMode::External,
        },
    };
    // Persist via StoredBackendConfig
    // ...
});
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Add `ServiceMode` enum + `ServiceConfig` struct to `bridge.slint` | existing file |
| 2 | Create `ui/state/service_config.slint` with `ServiceConfigBridge` | new file |
| 3 | Register in `ui/state/index.slint` and re-export from `main.slint` | existing files |
| 4 | Create `ui/pages/service_config_page.slint` | new file |
| 5 | Add `service-config` to the `Panel` enum | `bridge.slint` |
| 6 | Add conditional in `PanelHost` section of `main.slint` | existing file |
| 7 | Wire `on_save_config` callback in Rust | `lifecycle.rs` or new file |
| 8 | Add navigation entry in settings page to open `Panel.service-config` | `settings_page.slint` |
