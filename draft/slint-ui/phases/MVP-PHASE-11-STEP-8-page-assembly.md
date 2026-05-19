# MVP-PHASE-11 — Step 8: `MixerPage` assembly (header, scroll shell, Start/Stop, sections)

> Part 8 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-7](./MVP-PHASE-11-STEP-7-rtmp-destination-section.md).
>
> **Doc-only.** Snippets are illustrative — no source-tree files are
> modified by reading this step.

---

## 0. Goal of this step

Compose the four private sub-components built in STEP-5/6/7
(`SrtSourceRow` ×2, `MixerSlotControls` ×2, `MixerCanvasControls`,
`RtmpDestinationRow`) into a single `export component MixerPage` that
fills the screen.

Page structure (top to bottom):

1. Header bar — "Mixer" title + "Done" close button + global
   `Bridge.mixer-state` indicator.
2. Body (`ScrollView`):
   - Canvas section (`MixerCanvasControls`).
   - Source A section (`SrtSourceRow` + `MixerSlotControls`).
   - Source B section (`SrtSourceRow` + `MixerSlotControls`).
   - Destination section (`RtmpDestinationRow`).
3. Footer — Start / Stop buttons. Start dims when
   `Bridge.mixer-state != idle`; Stop is enabled only when
   `Bridge.mixer-state == running`.

> **Slint-doc references:**
>
> - [`reference/std-widgets/views/scrollview.mdx`](../docs/astro/src/content/docs/reference/std-widgets/views/scrollview.mdx)
>   §`mouse-drag-pan-enabled`.
> - [`reference/std-widgets/basic-widgets/button.mdx`](../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/button.mdx)
>   §Properties → `enabled`, `clicked`. (We use the in-tree
>   `PrimaryButton` / `DestructiveButton` wrappers, which proxy these.)
> - [`guide/language/coding/positioning-and-layouts.mdx`](../docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx)
>   §"VerticalLayout / HorizontalLayout" + §"Explicit vs automatic
>   layout".

---

## 1. The `MixerPage` component

Add to `ui/pages/mixer_page.slint`, at the bottom of the file:

```slint
export component MixerPage inherits Rectangle {
    width: 100%;
    height: 100%;
    background: Theme.surface-primary;

    // Dirty tracker — set on any sub-component `edited()` fire.
    // Used only as a forward extension point; current implementation
    // leaves the Start button gated on Bridge.mixer-state only.
    property <bool> any-edits-pending: false;

    function reset-dirty() { root.any-edits-pending = false; }

    VerticalLayout {
        // ── Header ──────────────────────────────────────────────────
        Rectangle {
            height: 56px;
            background: Theme.surface-card;
            HorizontalLayout {
                padding: Theme.padding-screen;
                spacing: Theme.spacing-default;
                Text {
                    text: @tr("Mixer");
                    color: Theme.text-primary;
                    font-size: Theme.font-size-heading;
                    vertical-alignment: center;
                    horizontal-stretch: 1;
                }
                // Global mixer-state indicator. Same idle/starting/
                // running/stopping/error pattern as the per-source
                // status text in SrtSourceRow.
                Text {
                    text:
                        Bridge.mixer-state == MixerState.running  ? @tr("live")     :
                        Bridge.mixer-state == MixerState.starting ? @tr("starting") :
                        Bridge.mixer-state == MixerState.stopping ? @tr("stopping") :
                        Bridge.mixer-state == MixerState.error    ? @tr("error")    :
                                                                     @tr("idle");
                    color:
                        Bridge.mixer-state == MixerState.running ? Theme.success :
                        Bridge.mixer-state == MixerState.error   ? Theme.error-fg :
                                                                    Theme.text-secondary;
                    font-size: Theme.font-size-label;
                    vertical-alignment: center;
                }
                TextButton {
                    label: @tr("close-panel-button" => "Done");
                    clicked => { Bridge.active-panel = Panel.none; }
                }
            }
        }

        // ── Body ────────────────────────────────────────────────────
        ScrollView {
            mouse-drag-pan-enabled: true;
            VerticalLayout {
                spacing: Theme.spacing-default;
                padding: Theme.padding-screen;

                // Global error banner — rolled up from mixer-error-text.
                if Bridge.mixer-error-text != "": Rectangle {
                    background: Theme.error;
                    border-radius: Theme.radius-card;
                    min-height: 40px;
                    Text {
                        text: Bridge.mixer-error-text;
                        color: Theme.text-primary;
                        font-size: Theme.font-size-label;
                        wrap: word-wrap;
                        x: Theme.padding-card;
                        width: parent.width - Theme.padding-card * 2;
                        vertical-alignment: center;
                    }
                }

                // ── Canvas ──────────────────────────────────────────
                SettingsSection {
                    title: @tr("CANVAS");
                    MixerCanvasControls {
                        canvas-edited => { root.any-edits-pending = true; }
                    }
                }

                // ── Source A ────────────────────────────────────────
                SettingsSection {
                    title: @tr("SOURCE A");
                    SrtSourceRow {
                        title: @tr("Source A");
                        slot-label: "a";
                        data <=> Bridge.srt-source-a;
                        edited => { root.any-edits-pending = true; }
                    }
                    MixerSlotControls {
                        title: @tr("Mix — Source A");
                        data <=> Bridge.srt-source-a;
                    }
                }

                // ── Source B ────────────────────────────────────────
                SettingsSection {
                    title: @tr("SOURCE B");
                    SrtSourceRow {
                        title: @tr("Source B");
                        slot-label: "b";
                        data <=> Bridge.srt-source-b;
                        edited => { root.any-edits-pending = true; }
                    }
                    MixerSlotControls {
                        title: @tr("Mix — Source B");
                        data <=> Bridge.srt-source-b;
                    }
                }

                // ── Destination ─────────────────────────────────────
                SettingsSection {
                    title: @tr("DESTINATION");
                    RtmpDestinationRow {
                        edited => { root.any-edits-pending = true; }
                    }
                }

                // Some breathing room above the footer.
                Rectangle { height: 96px; background: transparent; }
            }
        }

        // ── Footer ──────────────────────────────────────────────────
        Rectangle {
            height: 96px;
            background: Theme.surface-bar;
            HorizontalLayout {
                padding: Theme.padding-screen;
                spacing: Theme.spacing-default;
                PrimaryButton {
                    label: @tr("Start");
                    enabled: Bridge.mixer-state == MixerState.idle
                          || Bridge.mixer-state == MixerState.error;
                    clicked => {
                        Bridge.start-mixer-cast();
                        root.reset-dirty();
                    }
                    horizontal-stretch: 1;
                }
                DestructiveButton {
                    label: @tr("Stop");
                    enabled: Bridge.mixer-state == MixerState.running
                          || Bridge.mixer-state == MixerState.starting;
                    clicked => { Bridge.stop-mixer-cast(); }
                    horizontal-stretch: 1;
                }
            }
        }
    }
}
```

### 1.1 Why the `<=>` on the `data` binding into the row

`SrtSourceRow.data` is declared `in-out property <SrtSource>` (STEP-5
§1.2). The page wants the row to mutate the Bridge struct directly —
not a copy of it — so the binding must be two-way. With a one-way
`data: Bridge.srt-source-a;`, the row would write into its local copy
and the Bridge struct would never see the changes. See
[`properties.mdx`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx)
§"Two-way bindings" — same reason `value <=> root.data.uri` is used
inside `LineEdit`.

### 1.2 Why `MixerCanvasControls` does not take a `data` binding

`MixerCanvasControls` binds to the global `Bridge.mixer-canvas`
directly (STEP-6 §2.2). The page does not need to thread the data in —
the singleton lookup happens inside the sub-component. This makes the
`MixerCanvasControls` declaration site cleaner.

### 1.3 Start-button gating

`enabled: Bridge.mixer-state == MixerState.idle || Bridge.mixer-state
== MixerState.error;` lets the user retry after a failed Start. The
button is **not** gated on `Bridge.rtmp-destination.uri != ""`
because the page should not silently swallow a missing field — the
Rust handler will return an error (`mixer-error-text`) and the global
banner at the top of the body will display it. (PHASE-12 can add
"hard validation" later if the team decides the silent-fail-with-banner
flow is too forgiving.)

### 1.4 Why `horizontal-stretch: 1;` on the two footer buttons

In a `HorizontalLayout`, children with `horizontal-stretch: 1`
distribute remaining space equally. With both buttons set to 1, the
two halves of the footer divide evenly. Without the stretch, the
buttons collapse to their intrinsic width (the label text plus
padding) and float to the left.

> **Slint-doc reference:**
> [`positioning-and-layouts.mdx`](../docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx)
> §"Stretching elements in layouts" (the property is called
> `horizontal-stretch` in `HorizontalLayout` and `vertical-stretch` in
> `VerticalLayout`).

### 1.5 Why the body is a `ScrollView` (not a `ListView`)

The body is a small set of fixed sub-components (1 canvas + 2 sources
+ 1 destination = 4 sections, never more). `ListView` is for **long
homogeneous repeating** content with virtualization. Using `ListView`
here would actually be *wrong* — the `ListView` widget assumes its
children are all instances of the same component, which our 4
heterogeneous sections are not. See
[`reference/std-widgets/views/listview.mdx`](../docs/astro/src/content/docs/reference/std-widgets/views/listview.mdx)
§"A ListView is like a Scrollview but it should have a `for` element".

`ci/ui-validate.sh` would also flag a nested `ListView` inside a
`ScrollView` (kills virtualization), so the `ScrollView` choice here
is the only correct one.

### 1.6 Why the 96px breathing-room `Rectangle` at the bottom

The footer is a fixed-height bar at the bottom of the page, **outside**
the `ScrollView`. When the body is scrolled to the bottom, the last
content row would otherwise butt up against the footer. The 96px
filler `Rectangle` gives the last `SettingsSection` 96px of
clear space above the footer, matching the footer's 96px height —
common Material-style "lift" gap pattern.

---

## 2. Expected diff size

About **160 lines added** to `ui/pages/mixer_page.slint` (the
`MixerPage` body alone). Cumulative file size after STEP-5/6/7/8:
~490 lines, comparable to `ui/pages/network_page.slint` (~210 lines)
× 2 sections of richer content — within typical FCast page size.

---

## 3. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build
```

Specific audits this step will trip if done wrong:

1. **Touch-target audit.** The Theme's `row-height` is 48px (PHASE-2
   token). The PrimaryButton/DestructiveButton inherit
   `height: Theme.row-height` (`ui/components/buttons.slint:14`). The
   header `Done` button uses `TextButton` (same height). Confirmed
   ≥48px on every interactive child.
2. **Panel orphan audit.** Because STEP-4 added the
   `Bridge.active-panel = Panel.mixer` set-site and the
   `if Bridge.active-panel == Panel.mixer: MixerPage` route, no
   orphans should be flagged. If the audit reports
   `Panel.mixer routed but no setter`, re-check STEP-4 §3.
3. **`animate ... { duration: 0 }`** audit — not triggered by this
   step (no animations introduced).
4. **`@tr(...)` audit.** Every user-visible string in §1 is wrapped
   in `@tr(...)`. The
   [`guide/development/translations.mdx`](../docs/astro/src/content/docs/guide/development/translations.mdx)
   guide is the source of truth — `@tr("…")` is the only acceptable
   pattern. PHASE-9's `PHASE-9-localization.md` lists which keys must
   land in the `.po` files for each language.

### 3.1 Manual smoke test (UI thread only, no Rust handlers)

After landing STEP-2..STEP-8 and rebuilding:

1. Launch the app. Tap a debug or settings entry that opens
   `Bridge.active-panel = Panel.mixer` (the row from STEP-4 §3).
2. Confirm the Mixer page renders with the canvas defaults (1280×720,
   44100 Hz) and empty URL/stream-key fields.
3. Type into the URL field, drag a slider, toggle a Switch — all
   inputs accept input.
4. Tap Start. Nothing observable happens at the runtime level (no Rust
   handler registered), but the Slint logs `Bridge.start-mixer-cast
   called but no handler wired` (STEP-3 §3 documents the warning).
5. Tap Done — the page closes and `Bridge.active-panel` returns to
   `Panel.none`.

If any of steps 1–5 fail, the bug is in STEP-2..STEP-8, not in
PHASE-12's Rust handlers (which do not yet exist).

---

## 4. Exit gate

- [ ] `export component MixerPage inherits Rectangle` declared.
- [ ] Header / body / footer all present, in order.
- [ ] All four sub-components from STEP-5/6/7 are instantiated.
- [ ] Start/Stop buttons gate on `Bridge.mixer-state`.
- [ ] Done button closes the panel.
- [ ] `cargo build` passes.
- [ ] `ci/ui-validate.sh --no-build` passes (touch-target,
      panel-routing, `@tr`, animate-duration-zero).
- [ ] Manual smoke test §3.1 passes.

Proceed to [STEP-9](./MVP-PHASE-11-STEP-9-rust-handler-reference.md).
