# STEP 07 — Refactor Media Backend Page

**Phase:** 3 (Independent UI Layer)
**Modified file:** `ui/pages/media_backend_page.slint`

---

## Goal

Update the existing Media Backend page to consume the new
`ServiceBridge` / `ServiceConfigBridge` globals instead of hard-coding
`Bridge.gstpop-*` references.  Make service-control sections conditional
based on the service configuration.

---

## 1. Replace direct Bridge references with ServiceBridge

The current page reads `Bridge.gstpop-service-state` and calls
`Bridge.start-gstpop-service()`.  Replace these with the generic
`ServiceBridge` equivalents.

### Before (current code, lines 131-187)

```slint
SettingsSection {
    title: @tr("SERVICE");
    visible: Bridge.media-backend == MediaBackendKind.gst-pop;
    // ... reads Bridge.gstpop-service-state ...
    // ... calls Bridge.start-gstpop-service() ...
}
```

### After

```slint
import { ServiceBridge, ServiceEntry, MediaOp } from "../state/service_bridge.slint";

// Replace the hard-coded SERVICE section with a generic service list.
for svc in ServiceBridge.services: SettingsSection {
    title: svc.label;
    visible: svc.enabled;

    Rectangle {
        background: Theme.surface-card;
        border-radius: Theme.radius-card;
        min-height: 96px;
        VerticalLayout {
            padding: Theme.padding-screen;
            spacing: 12px;

            HorizontalLayout {
                spacing: 8px;
                // Status dot — reuses the same colour logic
                Rectangle {
                    width: 12px;
                    height: 12px;
                    border-radius: 6px;
                    background:
                        svc.running && svc.healthy ? Theme.success :
                        svc.running                ? Theme.warning :
                        !svc.healthy               ? Theme.error-fg :
                                                     Theme.text-disabled;
                    y: (parent.height - self.height) / 2;
                }
                Text {
                    text: svc.status-text;
                    color: Theme.text-primary;
                    font-size: Theme.font-size-body;
                    horizontal-stretch: 1;
                    vertical-alignment: center;
                }
            }

            if svc.error-text != "": Text {
                text: svc.error-text;
                color: Theme.error-fg;
                font-size: Theme.font-size-label;
                wrap: word-wrap;
            }

            HorizontalLayout {
                spacing: 12px;
                PrimaryButton {
                    label: @tr("Start");
                    enabled: !svc.running;
                    clicked => {
                        ServiceBridge.request-service-op(svc.id, MediaOp.start);
                    }
                }
                DestructiveButton {
                    label: @tr("Stop");
                    enabled: svc.running;
                    clicked => {
                        ServiceBridge.request-service-op(svc.id, MediaOp.stop);
                    }
                }
            }
        }
    }
}
```

## 2. Add navigation to Service Config page

Below the service list, add a button that opens the config page:

```slint
TextButton {
    label: @tr("Service Configuration\u{2026}");
    clicked => {
        PanelBridge.push(Panel.service-config);
    }
}
```

## 3. Add StatusPill fallback for no-service mode

If no services are enabled, show a helpful message:

```slint
if !ServiceBridge.any-service-ready: Rectangle {
    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 56px;
    HorizontalLayout {
        padding: Theme.padding-screen;
        Text {
            text: @tr("No service is currently running. Open Service Configuration to enable one.");
            color: Theme.text-secondary;
            font-size: Theme.font-size-body;
            wrap: word-wrap;
        }
    }
}
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Import `ServiceBridge`, `ServiceEntry`, `MediaOp` at top of page | `media_backend_page.slint` |
| 2 | Replace hard-coded SERVICE section with `for svc in ServiceBridge.services` loop | same file |
| 3 | Keep the BACKEND engine selector section unchanged (it still controls `BackendKind`) | same file |
| 4 | Add "Service Configuration" navigation button | same file |
| 5 | Add no-service fallback text block | same file |
| 6 | Verify `slint-viewer ui/pages/media_backend_page.slint --component MediaBackendPage` renders | terminal |

---

## Notes

* The engine selector (Migration vs gst-pop) stays on this page.  The
  service toggles (enabled/disabled, mode) live on the new Service Config
  page.  This separation avoids a cluttered single page.
* The gst-pop-specific fields (URL, API key, pipeline ID) remain visible
  only when `Bridge.media-backend == MediaBackendKind.gst-pop`.  They are
  orthogonal to the service lifecycle.
