# MVP-PHASE-11 — Step 7: `RtmpDestinationRow` private sub-component

> Part 7 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-6](./MVP-PHASE-11-STEP-6-mix-controls-section.md).
>
> **Doc-only.** Snippets are illustrative — no source-tree files are
> modified by reading this step.

---

## 0. Goal of this step

Build the `RtmpDestinationRow` private sub-component used once at the
bottom of `MixerPage`. It exposes:

- enable toggle (`Switch`),
- URL field (`LineEdit`),
- stream-key field (`LineEdit` with `input-type: password`),
- a derived "full publish URL" preview row (read-only, helps the user
  see exactly what will be sent to the migration runtime),
- a `last-error` footer.

The Moblin reference is
`draft/moblin-ui/Moblin/View/Settings/Streams/Stream/Wizard/NetworkSetup/MyServers/StreamWizardNetworkSetupMyServersRtmpSettingsView.swift`
(URL + stream key + footer error label).

> **Slint-doc reference:**
> [`reference/std-widgets/views/lineedit.mdx`](../docs/astro/src/content/docs/reference/std-widgets/views/lineedit.mdx)
> §Properties → `input-type` (password masking).

---

## 1. The change

Add to `ui/pages/mixer_page.slint` immediately below the
`MixerCanvasControls` declaration from STEP-6:

```slint
// Private — RTMP egress row. Bound to Bridge.rtmp-destination.
component RtmpDestinationRow inherits Rectangle {
    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 240px;

    callback edited();

    // Computed preview of the full URL the migration runtime will see.
    // Read-only — only used inside this component to render the "Full
    // publish URL" preview row. See §1.2 for the join semantics.
    pure function full-uri() -> string {
        if Bridge.rtmp-destination.stream-key == "" {
            return Bridge.rtmp-destination.uri;
        }
        if Bridge.rtmp-destination.uri == "" {
            return "";
        }
        return Bridge.rtmp-destination.uri + "/" + Bridge.rtmp-destination.stream-key;
    }

    VerticalLayout {
        padding-left:   Theme.padding-screen;
        padding-right:  Theme.padding-screen;
        padding-top:    Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing:        Theme.spacing-default;

        // ── Header ──────────────────────────────────────────────────
        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: @tr("RTMP destination");
                color: Theme.text-primary;
                font-size: Theme.font-size-heading;
                vertical-alignment: center;
                horizontal-stretch: 1;
            }
            Text {
                text:
                    Bridge.rtmp-destination.state == MixerState.running  ? @tr("live")     :
                    Bridge.rtmp-destination.state == MixerState.starting ? @tr("starting") :
                    Bridge.rtmp-destination.state == MixerState.stopping ? @tr("stopping") :
                    Bridge.rtmp-destination.state == MixerState.error    ? @tr("error")    :
                                                                            @tr("idle");
                color:
                    Bridge.rtmp-destination.state == MixerState.running ? Theme.success :
                    Bridge.rtmp-destination.state == MixerState.error   ? Theme.error-fg :
                                                                           Theme.text-secondary;
                font-size: Theme.font-size-label;
                vertical-alignment: center;
            }
            Switch {
                checked <=> Bridge.rtmp-destination.enabled;
                toggled() => { root.edited(); }
            }
        }

        // ── URL ─────────────────────────────────────────────────────
        Text {
            text: @tr("URL");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            placeholder-text: @tr("rtmp://arn03.contribute.live-video.net/app");
            text <=> Bridge.rtmp-destination.uri;
            edited(text) => { root.edited(); }
        }

        // ── Stream key (password input) ─────────────────────────────
        Text {
            text: @tr("Stream key");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            input-type: password;
            placeholder-text: @tr("live_…");
            text <=> Bridge.rtmp-destination.stream-key;
            edited(text) => { root.edited(); }
        }

        // ── Full URL preview (read-only) ────────────────────────────
        Text {
            text: @tr("Full publish URL");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        Rectangle {
            background: Theme.surface-bar;
            border-radius: Theme.radius-card;
            min-height: 40px;
            Text {
                text: root.full-uri() == "" ? @tr("(not set)") : root.full-uri();
                color: Theme.text-primary;
                font-size: Theme.font-size-label;
                horizontal-alignment: left;
                vertical-alignment: center;
                overflow: elide;
                x: Theme.padding-card;
                width: parent.width - Theme.padding-card * 2;
            }
        }

        // ── Error footer ────────────────────────────────────────────
        if Bridge.rtmp-destination.last-error != "": Text {
            text: Bridge.rtmp-destination.last-error;
            color: Theme.error-fg;
            font-size: Theme.font-size-label;
            wrap: word-wrap;
        }
    }
}
```

### 1.1 Why `input-type: password`

The
[`lineedit.mdx`](../docs/astro/src/content/docs/reference/std-widgets/views/lineedit.mdx)
docs document `input-type` taking an `InputType` enum value with
`password` masking input. Moblin's
`StreamWizardNetworkSetupMyServersRtmpSettingsView.swift` does **not**
mask the key (it's a plain `TextField`); Slint defaults match the
Android platform convention, which **does** mask. Diverging from
Moblin here is intentional.

### 1.2 Join semantics of `full-uri()`

The migration runtime's `DestinationFamily::Rtmp { uri: String }`
(`src/migration/protocol.rs:148-150`) accepts a single string. RTMP
publish URLs conventionally take the form `rtmp://host/app/streamkey`
where `app` is part of the base URL and `streamkey` is the per-stream
secret. The join `"{uri}/{stream-key}"` works because the base URL
already ends with `/app` (or `/app/`, in which case the join produces
`/app//key` — which `rtmp2sink` tolerates). A more robust join would
trim trailing `/` from `uri` and leading `/` from `stream-key`; for
PHASE-11 the simple concatenation is documented and the
implementation can be tightened later without breaking the data
model.

> **Slint-doc reference:**
> [`functions-and-callbacks.mdx`](../docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx)
> §"Declaring Functions" — `pure function` is the right modifier for
> a side-effect-free string transform (it lets Slint cache the result
> across paints).

### 1.3 Why a separate "Full publish URL" preview row

The user types `uri` and `stream-key` separately but the migration
runtime sees them joined. Showing the join lets the user verify the
result before tapping Start, mirroring what `UrlCopyView` does in
Moblin's `UrlsView.swift`. Combined with the password-masked
stream-key field, the preview leaks the secret back into plain text —
**this is intentional** (the preview is on the same screen as the
masked field; the user has already typed the key). If a future phase
wants the preview to mask the key too, swap `root.full-uri()` with a
masked version (`"{uri}/****"`).

### 1.4 Why the preview is a `Rectangle` + nested `Text` (not just `Text`)

`Text` does not draw a background; wrapping it in a `Rectangle` gives
us the rounded "code-block" appearance that makes it visually obvious
the row is read-only. The explicit `x: Theme.padding-card;` and
`width: parent.width - Theme.padding-card * 2;` are needed because the
`Text` sits outside any layout (the `Rectangle` is its parent, not a
`VerticalLayout`) — see
[`positioning-and-layouts.mdx`](../docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx)
§"Logical pixels and length".

---

## 2. Expected diff size

About **95 lines added** to `ui/pages/mixer_page.slint`.

---

## 3. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build
```

Failure modes specific to this step:

1. **`pure function` returning `string` but called inside a binding
   context that expects a different type.** Slint will say
   `expected int, got string` (or similar). Make sure
   `root.full-uri()` is only ever assigned to `Text.text:` (a
   `string`).
2. **`input-type: password` rejected.** If the pinned Slint fork
   pre-dates the `InputType` enum's password variant, the compiler
   will say `unknown enum variant`. Fall back to omitting the line
   — the field becomes unmasked. Document the regression in
   `draft/slint-ui/docs/current-fcast-slint-notes.md` and file a
   futo-fork upgrade as a follow-on phase.

---

## 4. Exit gate

- [ ] `RtmpDestinationRow` declared as `component … inherits
      Rectangle`.
- [ ] `uri`, `stream-key`, `enabled` are two-way-bound to
      `Bridge.rtmp-destination.*`.
- [ ] `pure function full-uri() -> string` exists and handles the
      `stream-key == ""` and `uri == ""` cases.
- [ ] `Full publish URL` preview row is read-only.
- [ ] Error footer is conditional on `last-error != ""`.
- [ ] `cargo build` passes.
- [ ] `ci/ui-validate.sh --no-build` passes.

Proceed to [STEP-8](./MVP-PHASE-11-STEP-8-page-assembly.md).
