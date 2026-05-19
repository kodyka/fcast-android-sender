# MVP-PHASE-11 — Step 2: Bridge data model (`SrtSource`, `RtmpDestination`, `MixerCanvas` + Bridge properties)

> Part 2 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-1](./MVP-PHASE-11-STEP-1-preflight-inventory.md).
>
> **Doc-only.** Snippets are illustrative — no source-tree files are
> modified by reading this step.

---

## 0. Goal of this step

Add the **typed data model** the new screen needs to `ui/bridge.slint`:

1. `struct SrtSource` — one SRT input (uri, latency, stream-id, enable
   flag, plus a few mix-time runtime fields the page reads but does not
   write).
2. `struct RtmpDestination` — the single RTMP egress (uri, stream-key,
   enable flag).
3. `struct MixerCanvas` — the shared mixer config (canvas size,
   sample-rate, plus a read-only `state` enum).
4. Four new Bridge properties that hold those structs.

This step adds **no callbacks** (those land in STEP-3) and **no
`Panel` variant** (that lands in STEP-4). It is intentionally
data-only so the diff is small and `ci/ui-validate.sh --no-build`
stays green on every commit.

> **Slint-doc reference:**
> [`guide/language/coding/structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
> §Structs + §Enums.

---

## 1. Pre-flight

| Component | Live location |
|---|---|
| `Bridge` global declaration | `ui/bridge.slint:139-258` (post-STEP-1 verification) |
| Existing struct precedents (`MacroStep`, `LogEntry`, `QuickAction`, `StatusItem`, `ReceiverItem`, `BitratePreset`, `NetworkInterface`) | `ui/bridge.slint:43-133` |
| Existing in-out property precedents | `ui/bridge.slint:165-220` |

The new structs follow the exact same layout convention as
`NetworkInterface` (a 4-field flat struct, no nested objects). The new
`in-out` properties follow the exact convention as
`Bridge.banner-message`, `Bridge.banner-visible` — single value, not a
list — because there is exactly **one** of each (one source A, one
source B, one destination).

---

## 2. The change

**File:** `ui/bridge.slint`

### 2.1 New `MixerState` enum

The page renders a connection indicator per source and a global "cast
running" badge. PHASE-11 declares the enum so the indicator can switch
on it; the Rust handler in PHASE-12 will be the authoritative writer
(STEP-3 §3.3 specifies the property qualifier as `in` for that reason).

Insert near the existing `enum` block (`ui/bridge.slint:12-39`),
immediately after `enum RecordingState`:

```slint
// ── PHASE-11 — Mixer / SRT-source connection lifecycle ──────────────
// Rust writes; Slint reads. Identical pattern to RecordingState.
export enum MixerState {
    idle,         // no graph live
    starting,     // commands sent, awaiting Started
    running,      // mixer + destinations Started
    stopping,     // teardown commands sent
    error,        // last command returned an error; see Bridge.mixer-error-text
}
```

> **Slint-doc reference:**
> [`structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
> §Enums: default value is always the first variant, hence the
> `idle`-first order.

### 2.2 New `SrtSource` struct

Insert in the structs section near `NetworkInterface`
(`ui/bridge.slint:128-138`):

```slint
// ── PHASE-11 — One SRT input descriptor ─────────────────────────────
// Slint owns: `enabled`, `uri`, `latency-ms`, `stream-id`,
// `mix-alpha`, `mix-zorder`, `mix-volume` (set via sliders/fields on
// the Mixer page, in-out so the page can read its own previous value
// after a slider stop).
//
// Rust owns: `slot-id`, `state`, `last-error` (the graph slot link-id
// once `connect` succeeds, the current MixerState, and the last
// error text from the migration runtime if any). Slint reads these,
// never writes them — the runtime is the source of truth.
//
// `volume` is `float` in 0..1 to match the mixer config key
// `audio::volume` used at connect time (see STEP-9 §3.3 and
// src/migration/protocol.rs MixerSlotInfo.volume).
export struct SrtSource {
    slot-id:    string,    // Rust-owned: empty until `connect` returns
    enabled:    bool,
    uri:        string,    // e.g. "srt://relay.example:9710?mode=listener"
    latency-ms: int,       // 0..8000; default 2000
    stream-id:  string,    // optional, blank if not set
    mix-alpha:  float,     // 0..1; default 1.0
    mix-zorder: int,       // 0..9; default 0 (A) or 1 (B)
    mix-volume: float,     // 0..1; default 1.0
    state:      MixerState,
    last-error: string,
}
```

> **Slint-doc reference:** struct field syntax is exactly the
> `identifier: type,` form from `structs-and-enums.mdx` §Structs. The
> trailing comma on the last field is permitted.

### 2.3 New `RtmpDestination` struct

Insert immediately after `SrtSource`:

```slint
// ── PHASE-11 — RTMP egress descriptor ───────────────────────────────
// Slint owns: `enabled`, `uri`, `stream-key`.
// Rust owns: `node-id`, `state`, `last-error`.
//
// The migration runtime's `DestinationFamily::Rtmp { uri }` consumes
// the concatenation `uri + "/" + stream-key` (or just `uri` if
// `stream-key` is empty) — see STEP-9 §3.4 for the exact JSON shape.
// We split URL and stream-key in the UI to match Moblin's
// `RtmpServerStreamSettingsView.swift` design; STEP-9 is where the
// Rust handler joins them.
export struct RtmpDestination {
    node-id:    string,
    enabled:    bool,
    uri:        string,    // "rtmp://live.example.com/app"
    stream-key: string,    // "live_48950233_okF4f455GRWEF443fFr23GRbt5rEv"
    state:      MixerState,
    last-error: string,
}
```

### 2.4 New `MixerCanvas` struct

Insert immediately after `RtmpDestination`:

```slint
// ── PHASE-11 — Shared canvas / sample-rate config ───────────────────
// The mixer's `createmixer` config dict (STEP-9 §3.1). Slint owns
// `width`, `height`, `sample-rate`; Rust owns `node-id`, `state`,
// `last-error`. Sensible defaults match the crossfade test in
// src/lib.rs:283-307 (1280x720, 44.1kHz).
export struct MixerCanvas {
    node-id:     string,
    width:       int,
    height:      int,
    sample-rate: int,
    state:       MixerState,
    last-error:  string,
}
```

### 2.5 New Bridge properties

Inside the `export global Bridge { … }` block, after the
PHASE-9 migration-runtime callbacks (`ui/bridge.slint:251-253`) and
before the `public function change-state` closing the block, add:

```slint
    // ── PHASE-11 — Mixer screen (SRT sources + RTMP destination) ────
    //
    // Naming convention: `srt-source-{a,b}` (one slot each, not a list)
    // matches the screen's information architecture — there are
    // exactly two sources, mixed onto one canvas, sent to one
    // destination. Use plural arrays only if a follow-on phase adds
    // a third source.
    in-out property <SrtSource>       srt-source-a: {
        slot-id:    "",
        enabled:    true,
        uri:        "",
        latency-ms: 2000,
        stream-id:  "",
        mix-alpha:  1.0,
        mix-zorder: 0,
        mix-volume: 1.0,
        state:      MixerState.idle,
        last-error: "",
    };
    in-out property <SrtSource>       srt-source-b: {
        slot-id:    "",
        enabled:    true,
        uri:        "",
        latency-ms: 2000,
        stream-id:  "",
        mix-alpha:  1.0,
        mix-zorder: 1,
        mix-volume: 1.0,
        state:      MixerState.idle,
        last-error: "",
    };
    in-out property <RtmpDestination> rtmp-destination: {
        node-id:    "",
        enabled:    true,
        uri:        "",
        stream-key: "",
        state:      MixerState.idle,
        last-error: "",
    };
    in-out property <MixerCanvas>     mixer-canvas: {
        node-id:     "",
        width:       1280,
        height:      720,
        sample-rate: 44100,
        state:       MixerState.idle,
        last-error:  "",
    };

    // Global rollup for the page header chrome. Derived in Rust, read
    // in Slint. `idle` until the user taps Start, then walks through
    // `starting → running → stopping → idle` (or `error`). Slint must
    // never write this — see PHASE-8 Cluster-A naming convention for
    // `in` (Rust → Slint) vs `in-out` (mutually writable).
    in property <MixerState> mixer-state:      MixerState.idle;
    in property <string>     mixer-error-text: "";
```

> **Slint-doc reference:**
> [`properties.mdx`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx)
> §"Properties" — `in` for Rust → Slint flow, `in-out` for mutually
> writable. The mixer-page `MixerPage` sub-components must **not** flip
> `mixer-state`; only Rust does. This restriction is the same one
> PHASE-8 Cluster-A enforces for `recording-state`.

### 2.6 Why structs with default-value-per-field initialization?

Slint allows struct-typed properties to be initialized with a struct
literal that names every field (`{ slot-id: "", enabled: true, … }`).
The default value of a struct is "all fields default-constructed"
([`structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
§"The default value of a struct"), but a `string` default is `""` and
an `int` default is `0`, which gives ugly initial values (`latency-ms:
0`, `mix-alpha: 0`) — sliders would clamp to the minimum and the page
header would show "0 ms". Explicit defaults at the property declaration
fix this.

> **Anti-pattern:** do **not** initialize per-field defaults inside the
> struct declaration. Slint structs do not support `field: type =
> default` syntax (only Rust structs do). The default lives on the
> *property*, not the *struct*.

### 2.7 Rust-side generated accessor names (forward reference)

Slint kebab-case → Rust snake_case auto-conversion (per
[`globals.mdx`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx)
§Rust tab) means the new props become:

| Slint name                | Rust setter/getter on `global::<Bridge>()` |
|---|---|
| `srt-source-a`            | `set_srt_source_a` / `get_srt_source_a` |
| `srt-source-b`            | `set_srt_source_b` / `get_srt_source_b` |
| `rtmp-destination`        | `set_rtmp_destination` / `get_rtmp_destination` |
| `mixer-canvas`            | `set_mixer_canvas` / `get_mixer_canvas` |
| `mixer-state`             | `set_mixer_state` (read-only on the Slint side, so no `get_*` strictly needed by Rust, but the binding generator emits both) |
| `mixer-error-text`        | `set_mixer_error_text` / `get_mixer_error_text` |

The corresponding struct types appear in Rust as
`crate::SrtSource`, `crate::RtmpDestination`, `crate::MixerCanvas`,
`crate::MixerState` (generated by `slint::include_modules!`).

These names are documented here **for STEP-9 only** — no Rust file is
edited in this phase.

---

## 3. Expected diff size

Approximately **70 lines added** to `ui/bridge.slint` (3 structs ≈ 30
lines, 6 property declarations ≈ 35 lines, comments ≈ 5 lines). No
lines removed. No other files touched.

---

## 4. Verification

```sh
ci/ui-validate.sh --no-build
```

The audit script:

- Checks the Slint compiler accepts the file (the `--no-build` mode
  still runs the Slint AST check via
  [`grep`-based smoke tests](../docs/development.md)).
- Confirms no `(min-)height: <Npx>` <48px regressions (none introduced
  here — this step is data-only).
- Confirms no `ListView`-inside-`ScrollView` regressions (none
  introduced).
- Confirms no orphan `Panel.X` introduced (STEP-2 adds zero `Panel`
  variants).

If the Slint AST check fails, the most likely culprit is a missing
comma between struct fields. Re-check §2.2–§2.4 against the
`NetworkInterface` precedent.

---

## 5. Exit gate

- [ ] All three structs (`SrtSource`, `RtmpDestination`, `MixerCanvas`)
      exist in `ui/bridge.slint` and parse.
- [ ] `MixerState` enum exists and `MixerState.idle` is the first
      variant.
- [ ] All four `in-out` and two `in` properties exist with the literal
      default values in §2.5.
- [ ] `cargo build -p android-sender --target aarch64-linux-android`
      still passes (the Slint compiler runs as part of `build.rs`).
- [ ] `ci/ui-validate.sh --no-build` passes.

Proceed to [STEP-3](./MVP-PHASE-11-STEP-3-bridge-callbacks.md).
