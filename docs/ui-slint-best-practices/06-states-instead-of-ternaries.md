# 06 — Replace long `?:` chains with `states [ … when … ]`

## Goal

Convert the multi-arm conditional-property idiom (`X ? a : Y ? b : Z ? c : d`)
into Slint `states [ … ]` blocks wherever the same discriminant drives
**two or more correlated properties** of the same element. This makes
the visual state machine explicit, enables transition animations for
free, and keeps the three-arm "color + label + icon" updates in sync.

## Findings

The clearest offender is `ui/pages/media_backend_page.slint:54–69` — a
four-arm ternary for the **dot color** repeated as a four-arm ternary
for the **label text**, both driven by `Bridge.media-backend-state`:

```slint
// Dot
background:
    Bridge.media-backend-state == MediaBackendState.ready ? Theme.success :
    Bridge.media-backend-state == MediaBackendState.probing ? Theme.warning :
    Bridge.media-backend-state == MediaBackendState.error ? Theme.error-fg :
    Theme.text-disabled;

// Label, a few lines below
text:
    Bridge.media-backend-state == MediaBackendState.ready ? @tr("Ready") :
    Bridge.media-backend-state == MediaBackendState.probing ? @tr("Probing…") :
    Bridge.media-backend-state == MediaBackendState.error ? @tr("Error") :
    @tr("Disconnected");
```

If a new state (say `MediaBackendState.starting`) is added, you must
remember to update **both** ternaries. The compiler doesn't help.

Other multi-arm ternaries that should become `states`:

- `mixer_page.slint:34–46` — `SrtSourceRow` status text & color (five
  arms: idle/starting/running/stopping/error).
- `recording_page.slint:117–123` — record-button color (four arms).
- `info_banner.slint:40–46` — banner background (four arms).
- `status_badges.slint:58–61` — badge foreground color (three arms).
- `network_page.slint:59–63` — kind-icon glyph (four arms).

## Slint docs reference

- [`states.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/states.mdx)
  — `states [ name when condition: { prop: val; … } ]`. Multiple
  properties switch atomically. `in` / `out` transitions add animations
  without touching the property bindings.
- [`properties.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/properties.mdx)
  — declarative bindings track dependencies automatically; manual
  `changed` re-emission is an anti-pattern (covered in step
  [07](./07-strict-property-directions.md)).

## Before — `media_backend_page.slint:50–87`

```slint
// Status pill (current)
Rectangle {
    background: Bridge.media-backend-state == MediaBackendState.error
        ? Theme.error.darker(35%) : Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 56px;
    HorizontalLayout {
        padding: Theme.padding-screen;
        spacing: 8px;
        Rectangle {            // ← dot
            width: 12px;
            height: 12px;
            border-radius: 6px;
            background:
                Bridge.media-backend-state == MediaBackendState.ready ? Theme.success :
                Bridge.media-backend-state == MediaBackendState.probing ? Theme.warning :
                Bridge.media-backend-state == MediaBackendState.error ? Theme.error-fg :
                Theme.text-disabled;
            y: (parent.height - self.height) / 2;
        }
        VerticalLayout {
            spacing: 2px;
            horizontal-stretch: 1;
            Text {            // ← label
                text:
                    Bridge.media-backend-state == MediaBackendState.ready ? @tr("Ready") :
                    Bridge.media-backend-state == MediaBackendState.probing ? @tr("Probing…") :
                    Bridge.media-backend-state == MediaBackendState.error ? @tr("Error") :
                    @tr("Disconnected");
                color: Theme.text-primary;
                font-size: Theme.font-size-body;
            }
            // … status-text / error-text Text elements …
        }
    }
}
```

## After — extract a `StatusPill` component with a `states` block

```slint
// ui/components/status_pill.slint  — NEW
//
// A single pill that visually represents a `MediaBackendState`.
// Three correlated properties (dot color, label, pill background) flip
// in lock-step via the `states` block. Optional fade-in animation when
// the state changes.

import { Theme } from "../theme.slint";
import { MediaBackend, MediaBackendState } from "../state/index.slint";

export component StatusPill inherits Rectangle {
    // Read-only — purely a presentation component.
    in property <MediaBackendState> state:        MediaBackend.state;
    in property <string>            status-text:  MediaBackend.status-text;
    in property <string>            error-text:   MediaBackend.error-text;

    border-radius: Theme.radius-card;
    min-height: 56px;
    background: Theme.surface-card;     // overridden in `error` state

    accessible-role:    text;
    accessible-label:   @tr("media-backend-status-a11y", "Media backend: {}", label-text);

    private property <color>  dot-color: Theme.text-disabled;
    private property <string> label-text: @tr("Disconnected");

    states [
        ready when root.state == MediaBackendState.ready : {
            root.dot-color:  Theme.success;
            root.label-text: @tr("Ready");
        }
        probing when root.state == MediaBackendState.probing : {
            root.dot-color:  Theme.warning;
            root.label-text: @tr("Probing\u2026");

            in { animate root.dot-color { duration: 200ms; } }
        }
        error when root.state == MediaBackendState.error : {
            root.dot-color:  Theme.error-fg;
            root.label-text: @tr("Error");
            root.background: Theme.error.darker(35%);

            in { animate root.background { duration: 200ms; } }
        }
        // Default state: `disconnected`. No `when` clause — Slint picks
        // this when no other guard fires.
        disconnected : {
            root.dot-color:  Theme.text-disabled;
            root.label-text: @tr("Disconnected");
        }
    ]

    HorizontalLayout {
        padding: Theme.padding-screen;
        spacing: 8px;

        Rectangle {
            width: 12px;
            height: 12px;
            border-radius: self.height / 2;
            background: root.dot-color;
            y: (parent.height - self.height) / 2;
        }

        VerticalLayout {
            spacing: 2px;
            horizontal-stretch: 1;

            Text {
                text: root.label-text;
                color: Theme.text-primary;
                font-size: Theme.font-size-body;
            }
            if root.status-text != "": Text {
                text: root.status-text;
                color: Theme.text-secondary;
                font-size: Theme.font-size-label;
                wrap: word-wrap;
            }
            if root.error-text != "": Text {
                text: root.error-text;
                color: Theme.error-fg;
                font-size: Theme.font-size-label;
                wrap: word-wrap;
            }
        }
    }
}
```

Call-site in `media_backend_page.slint` collapses to:

```slint
import { StatusPill } from "../components/status_pill.slint";

// …
StatusPill { }     // binds to MediaBackend.* by default
```

If a page wants to feed a pill from a different source (e.g. mixer page
showing recording status, or a test harness):

```slint
StatusPill {
    state:        Recording.state-as-media-equivalent();
    status-text:  Recording.label-for-state();
    error-text:   "";
}
```

## Before — `info_banner.slint:40–46`

```slint
background: root.severity == BannerSeverity.error
              ? Theme.error
          : root.severity == BannerSeverity.warning
              ? Theme.warning
          : root.severity == BannerSeverity.success
              ? Theme.success
          : Theme.accent-active.darker(20%);
```

## After — `info_banner.slint` with `states`

```slint
export component InfoBanner inherits Rectangle {
    in property <string>          message: BannerBridge.message;
    in property <BannerSeverity>  severity: BannerBridge.severity;
    in-out property <bool>        shown: BannerBridge.visible;

    height: root.shown ? 40px : 0px;
    clip: true;

    accessible-role:  alert;
    accessible-label: root.message;

    private property <color> banner-bg: Theme.accent-active.darker(20%);

    states [
        error   when root.severity == BannerSeverity.error   : { root.banner-bg: Theme.error;   }
        warning when root.severity == BannerSeverity.warning : { root.banner-bg: Theme.warning; }
        success when root.severity == BannerSeverity.success : { root.banner-bg: Theme.success; }
        info    when root.severity == BannerSeverity.info    : { root.banner-bg: Theme.accent-active.darker(20%); }
    ]

    background: root.banner-bg;
    animate height    { duration: 200ms; easing: ease-out; }
    animate background { duration: 150ms; easing: ease-in-out; }

    HorizontalLayout {
        padding-left:  Theme.padding-screen;
        padding-right: Theme.padding-screen;
        Text {
            text: root.message;
            color: Theme.text-on-accent;
            vertical-alignment: center;
            font-size: Theme.font-size-label;
        }
    }
}
```

## Before — `mixer_page.slint:34–46` (SrtSourceRow state)

```slint
Text {
    text:
        root.data.state == MixerState.idle ? @tr("idle")
        : root.data.state == MixerState.starting ? @tr("starting")
        : root.data.state == MixerState.running ? @tr("running")
        : root.data.state == MixerState.stopping ? @tr("stopping")
        : @tr("error");
    color:
        root.data.state == MixerState.running ? Theme.success
        : root.data.state == MixerState.error ? Theme.error-fg
        : Theme.text-secondary;
    font-size: Theme.font-size-label;
    vertical-alignment: center;
}
```

## After — `MixerStateChip` with `states`

```slint
// Inline component inside mixer_page.slint, or its own file.
component MixerStateChip inherits Text {
    in property <MixerState> state: MixerState.idle;
    font-size: Theme.font-size-label;
    vertical-alignment: center;

    states [
        idle     when root.state == MixerState.idle     : { text: @tr("idle");     color: Theme.text-secondary; }
        starting when root.state == MixerState.starting : { text: @tr("starting"); color: Theme.text-secondary; }
        running  when root.state == MixerState.running  : { text: @tr("running");  color: Theme.success;        }
        stopping when root.state == MixerState.stopping : { text: @tr("stopping"); color: Theme.text-secondary; }
        error    when root.state == MixerState.error    : { text: @tr("error");    color: Theme.error-fg;       }
    ]
}
```

…then `MixerStateChip { state: root.data.state; }` inside the row.

## Before — `recording_page.slint:117–123` record-button color

```slint
background:
    Bridge.recording-state == RecordingState.idle         ? #cc0000
    : Bridge.recording-state == RecordingState.recording  ? #cc0000
    : Bridge.recording-state == RecordingState.paused     ? Theme.warning
    : Bridge.recording-state == RecordingState.finalizing ? Theme.text-disabled
    : #cc0000;
```

## After — `states` (post step 01 — `#cc0000` is now `Theme.recording-dot`)

```slint
Rectangle {
    width: 96px;
    height: 96px;
    border-radius: 48px;

    states [
        idle       when Recording.state == RecordingState.idle       : { background: Theme.recording-dot;    }
        recording  when Recording.state == RecordingState.recording  : { background: Theme.recording-dot;    }
        paused     when Recording.state == RecordingState.paused     : { background: Theme.warning;          }
        finalizing when Recording.state == RecordingState.finalizing : { background: Theme.text-disabled;    }
    ]
}
```

## When *not* to use `states`

`states` is meant for **enum-driven** correlated property switches.
Don't reach for it when:

- Only **one** property changes. A single `bool ? a : b` ternary or a
  short two-arm `?: ` is already optimal.
- The discriminant is a continuous value (`width > 500 ? A : B`).
  Use a `?:` or `if` element instead.
- The branches each instantiate a **different element**. Use
  `if cond: ComponentA { } if !cond: ComponentB { }`.

## Migration

1. For each finding above, decide whether the switch deserves a
   dedicated *component* (`StatusPill`, `MixerStateChip`) or just an
   inline `states` block on the existing element. Rule of thumb: if the
   same switch appears twice, hoist it.
2. Add the `states [ … ]` block. Slint requires every state to set the
   same set of properties or fall back to defaults — verify with
   `slint-viewer` that nothing is unbound.
3. Add `in` / `out` transitions only where they add value. Default
   200 ms ease-out for hue changes, 150 ms ease-in-out for background
   swaps.
4. Re-render the page and verify the state graph behaves identically.

### Per-file checklist

| File                                       | Switch source                          | Hoist into component?              |
| ------------------------------------------ | -------------------------------------- | ---------------------------------- |
| `ui/pages/media_backend_page.slint`        | `MediaBackend.state`                   | Yes — `StatusPill`                 |
| `ui/components/info_banner.slint`          | `BannerBridge.severity`                | No — inline `states` on root       |
| `ui/pages/mixer_page.slint`                | `MixerState` per row                   | Yes — `MixerStateChip`             |
| `ui/pages/recording_page.slint`            | `Recording.state`                      | No — inline `states` on the rec ring |
| `ui/components/status_badges.slint`        | `item.severity` per badge              | No — inline `states` on `Badge`    |
| `ui/pages/network_page.slint`              | `data.kind == "wifi" / …`              | Defer to step [08](./08-typed-models-and-enums.md) — needs enum first |

## Out of scope

- Animating `text:` transitions. Slint does not animate string values
  natively; the `in { animate dot-color { … } }` block above animates
  only the colour switch, label flips instantly. That's intentional.
- Rewriting the std-widget components (their existing `states` blocks
  are upstream — leave them alone).

## Acceptance

- [ ] No FCast-authored `.slint` file has a property binding with
      **four or more arms** of the same `X == E.a ? : X == E.b ? : …`
      pattern.
      Verify with `grep -nE '== [A-Z]\w*\.[a-z]+ \?.*\?.*\?' ui/components/*.slint ui/pages/*.slint`
      — output should be empty (or only contain `std/` files).
- [ ] Each `states` block reads cleanly; `slint-viewer` does not warn
      about missing-binding-in-state.
- [ ] No visual regression on `MediaBackendPage`, `InfoBanner`,
      `MixerPage`, `RecordingPage`.
