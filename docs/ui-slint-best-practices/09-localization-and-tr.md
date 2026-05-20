# 09 — Localization: `@tr()` discipline and the ComboBox-equality bug

## Goal

Make every user-visible string translatable, fix the ComboBox selection
parsing in `media_backend_page.slint` (currently compares against
`@tr("Migration (in-process)")` — breaks under any non-English locale),
and adopt the upstream Slint best practice of `@tr("key", "{}", arg)`
substitutions over `+` concatenation.

## Findings

### F7 — `media_backend_page.slint:108–113` parses ComboBox value against a localised string

```slint
ComboBox {
    model: [@tr("Migration (in-process)"), @tr("gst-pop (WebSocket)")];
    current-index: Bridge.media-backend == MediaBackendKind.migration ? 0 : 1;
    selected(value) => {
        Bridge.media-backend = value == @tr("Migration (in-process)")
            ? MediaBackendKind.migration
            : MediaBackendKind.gst-pop;
        root.any-edits-pending = true;
    }
}
```

When the locale flips to (say) Spanish, the user picks
`"Migración (en proceso)"` and `value == @tr("Migration (in-process)")`
returns `false`, so the backend silently snaps to `gst-pop`. Worse —
this happens even when the user picks `gst-pop` in Spanish, because
*neither* arm of the ternary matches.

The fix is to dispatch on **index**, not on the translated string.

### F18 — Translation discipline

Scan the FCast pages:

- `git grep -nF '@tr(' ui/pages ui/components | wc -l` → 200+ hits.
  The vast majority correctly use `@tr("…")`. 
- A handful do `@tr("Hello, ") + name`-style concatenation, which the
  Slint best-practices guide warns against:

  > *Avoid `+` for concatenating strings, prefer `{}` substitutions.
  > This gives translators the option of re-ordering the arguments for
  > the most natural translation.*

- A handful use bare strings (placeholders for stubbed-out pages, e.g.
  `connect_page.slint:138 "Pair via QR"` is wrapped, but stub strings
  in `qr_placeholder.slint`, `network_page.slint:80
  "(no IPv4 address)"`, and `mixer_page.slint:59
  "srt://relay.example:9710?mode=caller"` are not wrapped).

- Several pages use `@tr("context" => "default")` for disambiguation —
  e.g. `@tr("close-panel-button" => "Done")` in 17 panel headers — but
  not consistently. Same word can mean very different things ("Open"
  the panel vs "Open" the value); discriminate every ambiguous string.

- `i18n/messages.pot` exists but no per-locale `.po` is checked in.

## Slint docs reference

- [Best Practices — Translations](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx#translations)
  — wrap early, prefer `{}` over `+`.
- [`translations.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/translations.mdx)
  — `@tr("context" => "msgid", args…)`, plural forms with
  `@tr("singular" | "plural" % count)`, context disambiguation.

## Before — `media_backend_page.slint:105–117`

```slint
ComboBox {
    model: [@tr("Migration (in-process)"), @tr("gst-pop (WebSocket)")];
    current-index: Bridge.media-backend == MediaBackendKind.migration ? 0 : 1;
    selected(value) => {
        Bridge.media-backend = value == @tr("Migration (in-process)")
            ? MediaBackendKind.migration
            : MediaBackendKind.gst-pop;
        root.any-edits-pending = true;
    }
}
```

## After — index-based ComboBox dispatch + helper function

```slint
// ui/pages/media_backend_page.slint  (target)
import { MediaBackend, MediaBackendKind } from "../state/index.slint";

// Order MUST match the model below.
property <[string]> backend-labels: [
    @tr("media-backend-engine-migration", "Migration (in-process)"),
    @tr("media-backend-engine-gstpop",     "gst-pop (WebSocket)"),
];

pure function backend-from-index(i: int) -> MediaBackendKind {
    return i == 0 ? MediaBackendKind.migration : MediaBackendKind.gst-pop;
}

pure function backend-to-index(k: MediaBackendKind) -> int {
    return k == MediaBackendKind.migration ? 0 : 1;
}

// …

ComboBox {
    model: root.backend-labels;
    current-index: backend-to-index(MediaBackend.kind);
    selected(value) => {
        // ComboBox 1.15 only emits `value: string`. Look the index up
        // from the model — this is the only correct way to dispatch.
        for label[i] in root.backend-labels:
            if label == value {
                MediaBackend.kind = backend-from-index(i);
            }
        root.any-edits-pending = true;
    }
}
```

> Slint 1.16+ adds `ComboBox.current-index-changed(index)` as a sibling
> of `selected(value)`. On 1.16+, prefer the index callback directly:
>
> ```slint
> current-index-changed(idx) => {
>     MediaBackend.kind = backend-from-index(idx);
>     root.any-edits-pending = true;
> }
> ```
>
> The fcast project is pinned to 1.15.1, so the `for … in label` lookup
> is the portable fallback.

### Why the lookup, not "if `value == @tr(...)`"?

Because the user could change locale at runtime and the `@tr()` result
shifts under your feet. The label list `backend-labels` is also
declarative — Slint re-evaluates `@tr()` calls in its body when the
locale changes, so `for label[i] in root.backend-labels: if label ==
value` always matches the active locale.

### Apply the same fix everywhere a ComboBox parses its `value`

`grep -nE 'selected\(value\)' ui/pages` → any spot using
`selected(value) => { … value == @tr(…) … }` is exposed to the same
bug. As of this branch:

- `media_backend_page.slint:108` — the only offender.

The other ComboBox-driven pages (`settings_page.slint`,
`bitrate_*_page.slint`) use `current-index <=> some-int-prop` and don't
parse `value`. Those are already correct.

## `@tr("Hello, ") + name` → `@tr("Hello, {}", name)` (where applicable)

`git grep -nE '@tr\([^,]+\)\s*\+\s*' ui/` returns a small set of hits:

```text
ui/components/control_bar.slint:37:                text: root.action.is-macro
                                                       ? "▶ " + root.action.title
                                                       : root.action.title;
```

This is a glyph + title prefix, not a translation issue per se, but the
prefix `▶` would benefit from being `@tr("macro-prefix-glyph", "\u25b6") + …`
*if* the prefix glyph itself ever needs locale-dependent override. For
now, leave it.

The actual sentence-level concatenation candidates are zero in the
current codebase — every user-visible string is built up with either
`@tr("…")` alone, or `@tr("…", arg)` substitution (already correct).

## Wrap the missing strings

`git grep -nE '"[A-Z][a-z]' ui/components ui/pages` shows a handful of
strings that should be wrapped:

- `qr_placeholder.slint:40 (QR preview)` → `@tr("qr-placeholder-label", "(QR preview)")`
- `network_page.slint:80 "(no IPv4 address)"` → `@tr("network-no-ipv4", "(no IPv4 address)")`
- `recording_page.slint:103 "(QR preview)"` etc. — audit each placeholder.
- `mixer_page.slint:59 "srt://relay.example:9710?mode=caller"` —
  placeholder URLs are NOT translated; this is correct.

Rule of thumb: if the string is **example data** (URLs, hashes,
codes), don't wrap. If it's **prose**, wrap.

## Context tags for ambiguity

`@tr("context" => "msgid")` disambiguates words that share spelling.
The codebase already does this for the four most common cases:

| Context tag                  | msgid       |
| ---------------------------- | ----------- |
| `close-panel-button`         | `"Done"`    |
| `dismiss-dialog-button`      | `"Cancel"`  |
| `open-panel-action`          | `"Open"`    |
| `save-button`                | `"Save"`    |

Extend the convention for new ambiguity:

- `connect-action`               => `"Connect"` (the connect-to-receiver button)
- `connecting-status`            => `"Connecting…"`
- `start-cast-button`            => `"Start"`
- `cancel-cast-button`           => `"Cancel"`
- `forget-receiver-button`       => `"Forget"`
- `stop-cast-button`             => `"Stop Casting"`
- `media-backend-engine-*`       => `"Migration (in-process)"`, `"gst-pop (WebSocket)"`
- `network-kind-glyph-*`         => single-letter glyphs per
  `NetworkKind`
- `destructive-button-a11y-label` => `"{} (destructive)"` (used by
  `DestructiveButton.accessible-label` from step 05)
- `media-backend-status-a11y`     => `"Media backend: {}"` (used by
  `StatusPill.accessible-label` from step 06)

## Plural forms

`@tr("singular" | "plural" % count)` already exists in
`settings_page.slint:136`:

```slint
value: @tr("{n} receiver found" | "{n} receivers found" % 3);
```

…but the `% 3` arg is a hard-coded `3`, not the actual receiver count.
This is presumably a stub. The target shape (after step 02 — `Receivers`
global):

```slint
value: @tr("{n} receiver found" | "{n} receivers found" % Receivers.devices.length);
```

Audit other counter sites:

- `network_page.slint` — interface count
- `cast_history_page.slint` — history count
- `bitrate_presets_page.slint` — preset count
- `macros_page.slint` — macro count
- `quick_actions_page.slint` — action count

All should use the plural form with the live count, not a stub.

## Slint docs reference (cont.)

The full plural-form syntax from
[`translations.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/translations.mdx):

```slint
text: @tr("{n} apple" | "{n} apples" % apple-count);
```

Translators in `.po` files supply per-language plural rules (`nplurals=2; plural=n!=1;`
for English, more complex for Russian/Arabic/etc).

## Migration

1. Fix `media_backend_page.slint` ComboBox first — it's a live
   correctness bug, not just style.
2. Sweep `git grep -nE '@tr\([^,]+\)\s*\+\s*' ui/` and convert any
   `@tr("X") + arg` to `@tr("X-with-arg", "X {}", arg)`.
3. Audit `git grep -nE '"[A-Z][a-z]' ui/` for un-wrapped strings; wrap
   each one with a context tag.
4. Replace hard-coded `% N` plural arguments with the live count.
5. Add a CI lint that runs `slint-tr-extractor ui/main.slint` (or
   manual `xgettext` over `messages.pot`) and warns on un-wrapped
   strings — see step [14](./14-testing-and-validation.md).

### Per-file checklist

| File                                       | Action                                                     |
| ------------------------------------------ | ---------------------------------------------------------- |
| `ui/pages/media_backend_page.slint`        | ComboBox: dispatch on index via `backend-from-index`       |
| `ui/components/qr_placeholder.slint`       | Wrap `(QR preview)` (and other placeholder strings)        |
| `ui/pages/network_page.slint`              | Wrap `(no IPv4 address)`; live plural count                |
| `ui/pages/cast_history_page.slint`         | Live plural count                                          |
| `ui/pages/bitrate_presets_page.slint`      | Live plural count                                          |
| `ui/pages/macros_page.slint`               | Live plural count                                          |
| `ui/pages/quick_actions_page.slint`        | Live plural count                                          |
| `ui/pages/settings_page.slint:136`         | Replace `% 3` with `% Receivers.devices.length`            |
| `ui/components/buttons.slint`              | `DestructiveButton.accessible-label` (from step 05) → ctx tag |
| `ui/components/status_pill.slint`         (from step 06) | `accessible-label` → ctx tag                  |
| `i18n/messages.pot`                        | Re-generate with `slint-tr-extractor ui/main.slint -o ui/i18n/messages.pot` |

## Out of scope

- Adding `.po` files for specific locales. Translation work is its own
  project; this step makes the *infrastructure* correct.
- BCP-47 negotiation, plural-rule tables, RTL layout. The current
  app is LTR-only and that's fine.
- Switching `@tr` to ICU MessageFormat. Slint doesn't support it; if a
  future locale needs gendered concord, escalate.

## Acceptance

- [ ] `git grep -nF "value == @tr" ui/` returns no hits.
- [ ] The Spanish locale (or any other locale that flips
      `"Migration (in-process)"`) round-trips the ComboBox selection
      correctly. Manually verify via `LANG=es_ES.UTF-8
      slint-viewer ui/main.slint`.
- [ ] `slint-tr-extractor ui/main.slint -o /tmp/extract.pot`
      followed by `diff /tmp/extract.pot ui/i18n/messages.pot` shows
      zero drift.
- [ ] No `@tr("…") + var` concatenation hits remain.
- [ ] Plural-form arguments use live counts, not literals.
