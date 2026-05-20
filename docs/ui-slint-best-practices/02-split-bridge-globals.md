# 02 — Split `Bridge` into per-feature globals

## Goal

Replace the 378-line `Bridge` god-singleton with a small set of
purpose-shaped globals (`AppBridge`, `MediaBackend`, `Recording`,
`Mixer`, `Macros`, `Quickbar`, `History`, `Network`, `BitratePresets`,
`Bridge` shrinks to a thin "shell" of cross-cutting properties). Each
global has a narrow, documented contract, and Rust call-sites bind to
the global that owns the data instead of fishing it out of a giant
object.

## Findings

`wc -l ui/bridge.slint` → 378 lines. `grep -c '^[[:space:]]*\(in\|in-out\|out\|callback\)' ui/bridge.slint` → 92 declarations on the `Bridge` global. The doc-block at the top of `main.slint:4–22` already documents that different *concerns* of `Bridge` have different writers, but they all sit on the same global.

Grouped by concern (each row = one future global):

| Concern             | Properties / callbacks in `Bridge` today                                                                                                                                                                                                                                  |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| App lifecycle       | `app-state`, `change-state(to)`, `lifecycle`, `engage-lock`, `engage-stealth`, `start-snapshot-countdown`, `exit-lifecycle`, `snapshot-secs`                                                                                                                                |
| Banner / toast      | `banner-message`, `banner-visible`, `banner-severity`                                                                                                                                                                                                                     |
| Panel routing       | `active-panel`, `back-requested`                                                                                                                                                                                                                                          |
| Receivers           | `devices`, `connect-receiver`, `selected-receiver-id`, `selected-receiver-name`                                                                                                                                                                                           |
| Casting             | `start-casting`, `stop-casting`, `status-items`                                                                                                                                                                                                                            |
| Recording           | `recording-state`, `recording-elapsed-s`, `start-recording`, `pause-recording`, `resume-recording`, `stop-recording`                                                                                                                                                       |
| Quick actions / macros | `quick-actions`, `move-bar-action`, `set-bar-action-enabled`, `save-bar-actions`, `invoke-action`, `macros`, `save-macro`, `delete-macro`, `run-macro`, `macro-edit-id`, `draft-macro-*`, `load-draft-macro`, `draft-add-step`, `draft-remove-step`, `draft-move-step`     |
| Cast history        | `history`, `selected-history-id`, `selected-history-entry`, `selected-history-id-changed`, `clear-history`, `delete-history-entry`, `recast`                                                                                                                              |
| Bitrate presets     | `presets`, `selected-preset-id`, `save-preset`, `delete-preset`, `set-active-preset`                                                                                                                                                                                       |
| Media backend       | `media-backend`, `media-backend-state`, `media-backend-status-text`, `media-backend-error-text`, `gstpop-url`, `gstpop-api-key`, `gstpop-pipeline-id`, `save-media-backend-settings`, `probe-media-backend`, `apply-media-backend`                                          |
| Migration runtime   | `start-migration-server`, `run-migration-test`, `stop-migration-server`                                                                                                                                                                                                    |
| Network             | `network-interfaces`, `set-interface-enabled`, `wifi-aware-enabled`, `set-wifi-aware`                                                                                                                                                                                       |
| Mixer               | `srt-source-a`, `srt-source-b`, `rtmp-destination`, `mixer-canvas`, `mixer-state`, `mixer-error-text`, `start-mixer-cast`, `stop-mixer-cast`, `apply-mixer-slot-config`, `apply-mixer-canvas`                                                                              |
| Debug log           | `log-entries`, `clear-log-entries`, `show-debug`, `test-status`                                                                                                                                                                                                            |
| Camera / audio      | `audio-source-idx`, `audio-muted`, `audio-input-gain`, `audio-bitrate-idx`, `camera-idx`, `resolution-idx`, `framerate-idx`, `camera-mirror-front`, `camera-stabilization`, `camera-tap-to-focus`, `camera-zoom-level`                                                     |
| Build info          | `app-version`                                                                                                                                                                                                                                                              |

That's **16** logical concerns crammed into one global. Each concern's
Rust binding lives in a different `senders/android/src/` module — which
means today every module has to do `ui.global::<Bridge>().set_*` for
unrelated state, making the Bridge surface effectively `pub` for every
subsystem.

## Slint docs reference

- [`globals.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/globals.mdx)
  — multiple globals are first-class. The Slint compiler generates one
  Rust trait per global; `App::global::<MediaBackend>()` gives you a
  typed handle that only exposes the media-backend surface. This is
  exactly what we want.
- [`best-practices.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx)
  — "separate code, UI, and assets" implies separating UI **state**
  surfaces by concern too.

## Before — `ui/bridge.slint` (excerpt)

```slint
// ui/bridge.slint (current, lines 196..378 collapsed for brevity)
export global Bridge {
    // ── App lifecycle ────────────────────────────────────────────
    in-out property <AppState> app-state: AppState.Disconnected;
    in-out property <Panel>    active-panel: Panel.none;
    in-out property <LifecycleMode> lifecycle: LifecycleMode.normal;
    in-out property <int>      snapshot-secs: 5;

    // ── Banner ───────────────────────────────────────────────────
    in-out property <string>          banner-message:  "";
    in-out property <bool>            banner-visible:  false;
    in-out property <BannerSeverity>  banner-severity: BannerSeverity.info;

    // ── Devices / casting ────────────────────────────────────────
    in property <[ReceiverItem]> devices: [];
    callback connect-receiver(string);
    callback start-casting(int, int, int);
    callback stop-casting();
    in property <[StatusItem]> status-items: [];

    // ── Recording ────────────────────────────────────────────────
    in property <RecordingState> recording-state: RecordingState.idle;
    in property <int>            recording-elapsed-s: 0;
    callback start-recording();
    callback pause-recording();
    callback resume-recording();
    callback stop-recording();

    // ── Media backend ────────────────────────────────────────────
    in-out property <MediaBackendKind> media-backend: MediaBackendKind.migration;
    in property <MediaBackendState> media-backend-state: MediaBackendState.disconnected;
    in property <string> media-backend-status-text: "";
    in property <string> media-backend-error-text: "";
    in-out property <string> gstpop-url: "ws://127.0.0.1:9000";
    in-out property <string> gstpop-api-key: "";
    in-out property <string> gstpop-pipeline-id: "0";
    callback save-media-backend-settings();
    callback probe-media-backend();
    callback apply-media-backend();

    // … and 10 other concerns … …
}
```

## After — split into per-feature globals, keep `Bridge` as a shell

```slint
// ui/state/lifecycle.slint
export global AppBridge {
    // Slint-only writes: change-state(); back-requested() is fired by
    // back-key-scope and consumed by Rust panel-stack handler.
    in-out property <AppState>       app-state: AppState.Disconnected;
    in-out property <LifecycleMode>  lifecycle: LifecycleMode.normal;
    in-out property <int>            snapshot-secs: 5;
    in property      <string>        app-version: "";

    callback back-requested();
    callback engage-lock();
    callback engage-stealth();
    callback start-snapshot-countdown(int);
    callback exit-lifecycle();

    public function change-state(to: AppState) {
        AppBridge.app-state = to;
    }
}
```

```slint
// ui/state/banner.slint
export global BannerBridge {
    in-out property <string>          message:  "";
    in-out property <bool>            visible:  false;
    in-out property <BannerSeverity>  severity: BannerSeverity.info;
}
```

```slint
// ui/state/panels.slint
export global PanelBridge {
    in-out property <Panel> active: Panel.none;
    in-out property <[Panel]> stack: [];   // <— back-stack (see step 11)

    public function push(p: Panel) {
        PanelBridge.stack = [PanelBridge.active, ...PanelBridge.stack];
        PanelBridge.active = p;
    }
    public function pop() {
        if PanelBridge.stack.length == 0 {
            PanelBridge.active = Panel.none;
        } else {
            PanelBridge.active = PanelBridge.stack[0];
            PanelBridge.stack = PanelBridge.stack[1..];   // 1.16+: array slice
        }
    }
}
```

```slint
// ui/state/media_backend.slint
export global MediaBackend {
    // ── Pure state (Rust → Slint) ───────────────────────────────
    in property <MediaBackendState> state:        MediaBackendState.disconnected;
    in property <string>            status-text:  "";
    in property <string>            error-text:   "";

    // ── Settings (Slint → Rust on `apply`) ──────────────────────
    in-out property <MediaBackendKind> kind:           MediaBackendKind.migration;
    in-out property <string>           gstpop-url:     "ws://127.0.0.1:9000";
    in-out property <string>           gstpop-api-key: "";
    in-out property <string>           gstpop-pipeline-id: "0";

    // ── Commands (Slint → Rust) ─────────────────────────────────
    callback save-settings();
    callback probe();
    callback apply();
}
```

```slint
// ui/state/recording.slint
export global Recording {
    in property <RecordingState> state:     RecordingState.idle;
    in property <int>            elapsed-s: 0;

    callback start();
    callback pause();
    callback resume();
    callback stop();
}
```

```slint
// ui/state/casting.slint
export global Casting {
    in property <[StatusItem]> status-items: [];

    callback start(scale-width: int, scale-height: int, max-framerate: int);
    callback stop();
}
```

```slint
// ui/state/receivers.slint
export global Receivers {
    in property <[ReceiverItem]> devices: [];

    in-out property <string> selected-id:   "";
    in-out property <string> selected-name: "";

    callback connect(string);
}
```

> Repeat the same pattern for `History`, `Macros`, `Quickbar`,
> `BitratePresets`, `Network`, `Mixer`, `DebugLog`, `Camera`, `Audio`,
> `Migration`. Each global stays under ~30 lines.

```slint
// ui/state/index.slint  — one barrel for the bridge crate
import { AppBridge }       from "lifecycle.slint";
import { BannerBridge }    from "banner.slint";
import { PanelBridge }     from "panels.slint";
import { MediaBackend }    from "media_backend.slint";
import { Recording }       from "recording.slint";
import { Casting }         from "casting.slint";
import { Receivers }       from "receivers.slint";
// … one import per global file …
export { AppBridge, BannerBridge, PanelBridge, MediaBackend, Recording,
         Casting, Receivers /* , History, Macros, … */ }
```

`ui/main.slint` then imports from the barrel:

```slint
import { AppBridge, PanelBridge, MediaBackend, Recording } from "state/index.slint";
export { AppBridge, PanelBridge, MediaBackend, Recording }
```

### Rust call-sites — mechanical rename

The Slint compiler generates one trait per global. Before:

```rust
// src/lib.rs (before)
let ui = MainWindow::new()?;
ui.global::<Bridge>().set_media_backend_state(MediaBackendState::Ready);
ui.global::<Bridge>().set_media_backend_status_text("connected".into());
ui.global::<Bridge>().on_probe_media_backend(|| { /* … */ });
ui.global::<Bridge>().on_start_recording(|| { /* … */ });
ui.global::<Bridge>().set_app_state(AppState::Connecting);
```

After:

```rust
// src/lib.rs (target)
let ui = MainWindow::new()?;

// ── Media backend (state + commands) ────────────────────────────
ui.global::<MediaBackend>().set_state(MediaBackendState::Ready);
ui.global::<MediaBackend>().set_status_text("connected".into());
ui.global::<MediaBackend>().on_probe(|| { /* … */ });

// ── Recording (commands) ────────────────────────────────────────
ui.global::<Recording>().on_start(|| { /* … */ });

// ── App lifecycle ───────────────────────────────────────────────
ui.global::<AppBridge>().set_app_state(AppState::Connecting);
```

Every getter/setter loses the `media_backend_` / `recording_` prefix
because it's now implicit in the global it lives on.

### What stays on the legacy `Bridge`?

Nothing. Once every concern has moved, delete the `export global Bridge`
declaration. Keep the **type** declarations (`enum AppState`,
`struct ReceiverItem`, etc.) at the top of `bridge.slint`, or pull them
out into `ui/state/types.slint` for symmetry.

## Migration

The split lands incrementally, **one concern per PR**, so that each
intermediate state still compiles. Suggested order:

1. **Media backend** (smallest, most isolated — already self-contained
   in `media_backend_page.slint`).
2. **Recording** (5 callbacks, 2 props — also small).
3. **Banner** (3 props — touches `info_banner.slint` only).
4. **Panel routing + lifecycle** — these two are coupled (back-key
   handler lives on `AppBridge`, panel-stack lives on `PanelBridge`).
   Coordinate with step [11](./11-back-stack-and-navigation.md).
5. **Receivers + Casting** (status overlay needs both).
6. **History, Bitrate, Macros, Quickbar** — settings-style globals.
7. **Network, Mixer, Migration, DebugLog, Camera, Audio** — long-tail.

For each PR:

1. Create the new global file under `ui/state/`.
2. Add its export to `ui/state/index.slint` (or create the barrel on
   the first PR).
3. **Re-export from `ui/bridge.slint`** as a transitional shim so
   existing consumers keep compiling:

   ```slint
   // ui/bridge.slint (transitional)
   import { MediaBackend } from "state/media_backend.slint";
   export { MediaBackend }
   ```

4. Migrate one page at a time to import from `state/index.slint`
   instead of `bridge.slint`.
5. Migrate the matching Rust call-site to `ui.global::<MediaBackend>()`.
6. Remove the property/callback from `Bridge` itself.
7. When the last consumer is migrated, remove the transitional re-export.

### Per-file checklist (delta only — files that need to *change* their import line)

| File                                       | New import                                       |
| ------------------------------------------ | ------------------------------------------------ |
| `ui/pages/media_backend_page.slint`        | `import { MediaBackend, … } from "../state/index.slint"` |
| `ui/pages/recording_page.slint`            | `import { Recording, … } from "../state/index.slint"` |
| `ui/pages/connect_page.slint`              | `import { Receivers, PanelBridge, … }`           |
| `ui/pages/cast_history_*.slint`            | `import { History, … }`                          |
| `ui/components/info_banner.slint`          | `import { BannerBridge, … }`                     |
| `ui/main.slint`                            | `import { AppBridge, PanelBridge, … }`           |
| `ui/components/control_bar.slint`          | `import { Quickbar, PanelBridge, … }`            |
| `ui/components/lock_overlay.slint`         | `import { AppBridge, … }`                        |
| `ui/components/snapshot_countdown.slint`   | `import { AppBridge, … }`                        |
| `ui/components/status_*.slint`             | `import { Casting, … }`                          |
| `ui/components/status_badges.slint`        | `import { Casting, … }`                          |

## Out of scope

- Renaming `LifecycleMode`, `AppState`, `Panel` etc. — those are types,
  not state.
- Removing the `mock-*` properties on `capture_preview.slint`,
  `qr_placeholder.slint`, `status_badges.slint`. They live on the
  component, not on `Bridge`, so this step doesn't change them.
- Changing Rust state-machine ownership of any of these properties.
  The Bridge split is a *surface* refactor; the writers stay the same.

## Acceptance

- [ ] `grep -c '^[[:space:]]*\(in\|in-out\|out\|callback\)' ui/bridge.slint`
      returns **0** (the file only re-exports types after step 16 of the
      sub-migration).
- [ ] `wc -l ui/state/*.slint` shows no single file over ~50 lines.
- [ ] Rust `cargo check --lib` passes after each sub-PR — verified by
      the CI `gstpop-smoke` and `build-android-arm64-debug` jobs.
- [ ] No `ui.global::<Bridge>()` references left in `senders/android/src/`.
