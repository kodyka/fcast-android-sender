# STEP 08 — Main UI Decoupling

**Phase:** 3 (Independent UI Layer)
**Modified file:** `ui/main.slint`

---

## Goal

Remove direct service dependencies from the main window.  The
`MainWindow` should work with `ServiceBridge` for all service
interactions and remain functional even when no service is running.

---

## 1. Add service-aware conditional rendering

Currently `main.slint` unconditionally shows pages that assume a backend
is connected (e.g. `WaitingForMediaView`, `CastingView`).  Gate these on
`ServiceBridge.any-service-ready`:

### Before (lines 140-144)

```slint
if Bridge.app-state == AppState.Disconnected:      ConnectView { }
if Bridge.app-state == AppState.Connecting:        ConnectingView { }
if Bridge.app-state == AppState.SelectingSettings: SettingsPageView { }
if Bridge.app-state == AppState.WaitingForMedia:   WaitingForMediaView { }
if Bridge.app-state == AppState.Casting:           CastingView { }
```

### After

```slint
import { ServiceBridge } from "state/service_bridge.slint";

if Bridge.app-state == AppState.Disconnected:      ConnectView { }
if Bridge.app-state == AppState.Connecting:        ConnectingView { }
if Bridge.app-state == AppState.SelectingSettings: SettingsPageView { }
// Only show media pages when at least one backend service is available.
if Bridge.app-state == AppState.WaitingForMedia && ServiceBridge.any-service-ready:
    WaitingForMediaView { }
if Bridge.app-state == AppState.WaitingForMedia && !ServiceBridge.any-service-ready:
    NoServiceView { }
if Bridge.app-state == AppState.Casting:
    CastingView { }
```

## 2. Create a lightweight `NoServiceView`

```slint
// Inline in main.slint, or extract to ui/pages/no_service_page.slint

component NoServiceView inherits Rectangle {
    background: Theme.surface-primary;
    VerticalLayout {
        alignment: center;
        spacing: Theme.spacing-default;
        padding: Theme.padding-screen;

        Text {
            text: @tr("No media service is running");
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            horizontal-alignment: center;
        }
        Text {
            text: @tr("Enable a service in Settings > Service Configuration to start casting.");
            color: Theme.text-secondary;
            font-size: Theme.font-size-body;
            horizontal-alignment: center;
            wrap: word-wrap;
        }
        PrimaryButton {
            label: @tr("Open Service Configuration");
            clicked => { PanelBridge.push(Panel.service-config); }
        }
    }
}
```

## 3. Register the new panel in PanelHost

Add the `ServiceConfigPage` conditional to the PanelHost block in
`main.slint` (alongside the other `if PanelBridge.active == ...` lines):

```slint
// main.slint  PanelHost section  (add after existing panels)
import { ServiceConfigPage } from "pages/service_config_page.slint";

if PanelBridge.active == Panel.service-config: ServiceConfigPage { }
```

## 4. Remove legacy direct Bridge writes

Audit `main.slint` and page files for any remaining
`Bridge.gstpop-service-state` or `Bridge.start-gstpop-service()` calls.
Replace them with `ServiceBridge.request-service-op(...)` equivalents.

A quick grep to verify:

```console
$ git grep -n 'Bridge\.gstpop-service' -- 'ui/'
$ git grep -n 'Bridge\.start-gstpop-service\|Bridge\.stop-gstpop-service' -- 'ui/'
```

Both should return zero results after this step.

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Import `ServiceBridge` in `main.slint` | top of file |
| 2 | Gate `WaitingForMediaView` on `ServiceBridge.any-service-ready` | conditional block |
| 3 | Add `NoServiceView` component | inline or new file |
| 4 | Add `ServiceConfigPage` to PanelHost conditionals | `main.slint` |
| 5 | Remove all `Bridge.gstpop-service-*` reads/writes from UI files | grep + edit |
| 6 | Verify `slint-viewer ui/main.slint` still renders | terminal |

---

## Notes

* `CastingView` remains unconditionally visible because casting is already
  in progress — the service was running when it started.  If the service
  dies mid-cast, the backend error handler should set
  `Bridge.app-state = AppState.Disconnected`.
* The `Bridge` global still exists and carries per-backend fields (URL,
  API key, etc.).  The decoupling removes *service lifecycle* coupling,
  not *configuration* coupling.
