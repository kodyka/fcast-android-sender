# STEP 05 — SRT Source Section

**Internal component for:** `ui/pages/test_functionality_page.slint`

---

## Goal

Build the `TestSrtSourceCard` component, adapted from `mixer_page.slint`
`SrtSourceRow` (lines 24-99) and the Moblin
`SrtlaServerSettingsView.swift` / `SrtlaServerStreamSettingsView.swift`
patterns.

---

## Design reference

### From `mixer_page.slint` `SrtSourceRow`

The existing component provides:
- Header row: title + `MixerStateChip` + `Switch` enable toggle
- URL `LineEdit` with placeholder
- Latency `SettingsSliderRow` (0–8000 ms)
- Stream ID `LineEdit` (optional)
- Error text display

### From `SrtlaServerSettingsView.swift`

| Moblin element | Purpose | Our adaptation |
|----------------|---------|----------------|
| `Toggle("Enabled", isOn:)` | Server on/off | `Switch { checked <=> data.enabled }` |
| `TextEditNavigationView("SRT port", ...)` | Port config | Part of URL in our model |
| `TextEditNavigationView("SRTLA port", ...)` | SRTLA port | Part of URL in our model |
| `CreateButtonView { }` | Add new stream | Single source in test page |
| `InfoBannerView("Disable the SRT(LA) server...")` | Warning | Can add similar banner |

### From `SrtlaServerStreamSettingsView.swift`

| Moblin element | Purpose | Our adaptation |
|----------------|---------|----------------|
| `NameEditView(name:)` | Stream name | Title is fixed "SRT Source" |
| `TextEditNavigationView("Stream id", ...)` | Stream ID | `LineEdit` with placeholder |
| `UrlsView(proto:, port:, streamId:)` | Publish URLs display | Not needed — we're receiving |
| `Image(systemName: "cable.connector")` | Connection indicator | `TestStateChip` with state |

---

## Component snippet

```slint
// test_functionality_page.slint — Internal SRT source card component.
//
// Adapted from mixer_page.slint SrtSourceRow (lines 24-99).
// Design ref: draft/moblin-ui/.../SrtlaServerSettingsView.swift
//             draft/moblin-ui/.../SrtlaServerStreamSettingsView.swift

component TestStateChip inherits Text {
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

component TestSrtSourceCard inherits Rectangle {
    in-out property <SrtSource> data;
    callback edited();

    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 280px;

    VerticalLayout {
        padding-left: Theme.padding-screen;
        padding-right: Theme.padding-screen;
        padding-top: Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing: Theme.spacing-default;

        // ── Header: title + state chip + enable toggle ────────────────
        // Mirrors SrtSourceRow header layout exactly.
        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: @tr("SRT Source");
                color: Theme.text-primary;
                font-size: Theme.font-size-heading;
                vertical-alignment: center;
                horizontal-stretch: 1;
            }
            TestStateChip { state: root.data.state; }
            Switch {
                checked <=> root.data.enabled;
                toggled() => { root.edited(); }
            }
        }

        // ── URL input ─────────────────────────────────────────────────
        // Corresponds to SrtlaServerSettingsView's SRT/SRTLA port fields,
        // but we use a single URL since our SRT source is a caller.
        Text {
            text: @tr("URL");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            placeholder-text: @tr("srt://relay.example:9710?mode=caller");
            text <=> root.data.uri;
            edited(text) => { root.edited(); }
        }

        // ── Latency slider ───────────────────────────────────────────
        // Same as mixer_page SrtSourceRow latency control.
        SettingsSliderRow {
            title: @tr("Latency");
            unit: @tr(" ms");
            minimum: 0;
            maximum: 8000;
            show-fractional: false;
            value: root.data.latency-ms;
            changed(v) => {
                root.data.latency-ms = v;
                root.edited();
            }
        }

        // ── Stream ID ────────────────────────────────────────────────
        // Maps to SrtlaServerStreamSettingsView's TextEditNavigationView
        // for stream ID (alphanumeric).
        Text {
            text: @tr("Stream ID (optional)");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            placeholder-text: @tr("publish:my-stream-key");
            text <=> root.data.stream-id;
            edited(text) => { root.edited(); }
        }

        // ── Mix controls ─────────────────────────────────────────────
        // Adapted from MixerSlotControls component (mixer_page.slint
        // lines 101-163). Alpha and volume needed when compositing
        // with camera + overlay.
        SettingsSliderRow {
            title: @tr("Alpha");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> root.data.mix-alpha;
        }

        SettingsSliderRow {
            title: @tr("Volume");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> root.data.mix-volume;
        }

        // ── Error display ────────────────────────────────────────────
        if root.data.last-error != "": Text {
            text: root.data.last-error;
            color: Theme.error-fg;
            font-size: Theme.font-size-label;
            wrap: word-wrap;
        }
    }
}
```

---

## Usage in the page

```slint
// Inside the ScrollView VerticalLayout:
SettingsSection {
    title: @tr("SRT SOURCE");

    TestSrtSourceCard {
        data <=> Bridge.test-srt-source;
        edited => { }
    }
}
```

---

## Wire-up checklist

| # | Action |
|---|--------|
| 1 | Add `TestStateChip` component at the top of `test_functionality_page.slint` |
| 2 | Add `TestSrtSourceCard` component below it |
| 3 | Use inside a `SettingsSection { title: @tr("SRT SOURCE"); }` block |
| 4 | Ensure `Bridge.test-srt-source` property exists (STEP 02) |
| 5 | Import `SrtSource` and `MixerState` from `bridge.slint` |

---

## Notes

* `TestStateChip` is a local duplicate of `MixerStateChip` from
  `mixer_page.slint`.  Both use the same `states [...]` pattern.
  If you want to share the component, extract it to
  `ui/components/state_chip.slint` — but this is optional and can be
  done in a later cleanup pass.
* The `data <=> Bridge.test-srt-source` two-way binding means the Slint
  UI and Rust backend can both read/write the SRT config.  Rust should
  only write `state` and `last-error`; the UI writes the rest.
* The `edited()` callback is provided for future dirty-tracking (same
  as `mixer_page.slint`'s `any-edits-pending` pattern).
