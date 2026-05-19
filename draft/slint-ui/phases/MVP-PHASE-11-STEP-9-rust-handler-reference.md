# MVP-PHASE-11 — Step 9: Rust handler reference (specification for PHASE-12)

> Part 9 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-8](./MVP-PHASE-11-STEP-8-page-assembly.md).
>
> **Doc-only.** No Rust code ships in this phase — this step is the
> spec the follow-on PHASE-12 will implement. The JSON shapes are
> grounded in the existing crossfade smoke test
> (`src/lib.rs:283-425`) and the `Command` enum
> (`src/migration/protocol.rs:50-127`).

---

## 0. Goal of this step

Document the **exact `run_graph_command(...)` ladder** each of the
four PHASE-11 Bridge callbacks (`start-mixer-cast`, `stop-mixer-cast`,
`apply-mixer-slot-config`, `apply-mixer-canvas`) must execute when
PHASE-12 wires them. Every JSON shape below is verbatim what the
in-process runtime (`crate::migration::runtime::try_handle_command_json`,
`src/migration/runtime.rs:349-356`) expects.

This step also documents the Rust-side state writeback pattern (how
the slot id, node id, and `MixerState` get pushed back into the
matching `Bridge.srt-source-{a,b}` / `Bridge.rtmp-destination` /
`Bridge.mixer-canvas` / `Bridge.mixer-state` properties).

---

## 1. Command enum lookup table

Slint callback → action verb passed to `run_graph_command(action, …)`
→ underlying `Command` variant:

| Callback (Slint side) | Verbs the Rust handler must dispatch |
|---|---|
| `Bridge.start-mixer-cast()` | `createmixer`, `createsource` ×2, `createdestination`, `connect` ×3 (A→mixer, B→mixer, mixer→dst), `start` ×4 (dst, mixer, srcA, srcB) |
| `Bridge.stop-mixer-cast()` | `disconnect` ×3, `remove` ×4 |
| `Bridge.apply-mixer-slot-config(slot_id, alpha, zorder, volume)` | `connect` (re-dispatch on the same `link_id` updates the config dict — see §3.5) |
| `Bridge.apply-mixer-canvas(w, h, sr)` | If `mixer-state == idle`, only stash; if running, the runtime currently has no live-resize support — the handler must `remove` the mixer + everything downstream and re-run §3.1–§3.4. |

All verbs come from
[`src/migration/protocol.rs:50-127`](../../src/migration/protocol.rs)'s
`Command` enum, which is `#[serde(rename_all = "lowercase")]` — that's
why we send `"createmixer"` (lowercase, no underscore) and not
`"create_mixer"` or `"CreateMixer"`.

> **Slint-doc reference:** the Rust-side `app.global::<Bridge>()`
> access pattern is
> [`guide/language/coding/globals.mdx`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx)
> §Rust tab — `app.global::<Bridge>().on_callback(|args| …)` and
> `app.global::<Bridge>().set_property(value)`.

---

## 2. Rust handler skeleton

PHASE-12 will add this to `src/lib.rs` near the existing PHASE-9
migration-server handlers (`src/lib.rs:2138-2185`). This phase only
documents the shape — **do not ship the code in PHASE-11**.

```rust
// PHASE-12 — Slint→Rust mixer callbacks. Documented in
// draft/slint-ui/phases/MVP-PHASE-11-STEP-9-rust-handler-reference.md.
//
// All four handlers must hand off to a worker thread before touching
// the migration runtime — they fire on the UI thread and
// run_graph_command blocks until the runtime acknowledges.
{
    let ui_weak = ui.as_weak();
    ui.global::<Bridge>().on_start_mixer_cast(move || {
        let ui_weak = ui_weak.clone();
        std::thread::spawn(move || {
            let outcome = run_start_mixer_cast(&ui_weak);
            // Surface the final MixerState + any error text via
            // ui.upgrade_in_event_loop. Detail in §4 below.
            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                apply_mixer_outcome(&ui, outcome);
            });
        });
    });
}

// Repeat the same shape for on_stop_mixer_cast, on_apply_mixer_slot_config,
// on_apply_mixer_canvas. See PHASE-9 STEP-2 for the precedent.
```

### 2.1 Reading struct properties back from Bridge

Inside `run_start_mixer_cast(ui_weak)` the handler reads the current
`Bridge.srt-source-a`, `Bridge.srt-source-b`, `Bridge.rtmp-destination`,
`Bridge.mixer-canvas` struct values. Because Slint serializes struct
property reads into a memberwise clone, the read is:

```rust
fn snapshot_inputs(ui: &MainWindow) -> MixerInputs {
    let bridge = ui.global::<Bridge>();
    MixerInputs {
        canvas: bridge.get_mixer_canvas(),
        src_a:  bridge.get_srt_source_a(),
        src_b:  bridge.get_srt_source_b(),
        dst:    bridge.get_rtmp_destination(),
    }
}
```

The handler must call `snapshot_inputs` from inside
`ui.upgrade_in_event_loop` (UI thread), pass the resulting plain Rust
struct into the worker thread, and **never** touch
`ui.global::<Bridge>()` from outside the event loop.

---

## 3. JSON ladder for `Bridge.start-mixer-cast()`

The order below mirrors `run_legacy_http_crossfade_test`
(`src/lib.rs:283-380`) but replaces the videogenerator slot source
with two real `createsource` calls and the `LocalPlayback` destination
with an `Rtmp` destination.

### 3.1 `createmixer`

```rust
run_graph_command("createmixer", json!({
    "id": mixer_id,                  // e.g. format!("mixer-{}", Uuid::new_v4())
    "config": {
        "width":       inputs.canvas.width,        // int from Bridge.mixer-canvas
        "height":      inputs.canvas.height,
        "sample-rate": inputs.canvas.sample_rate,
    },
    // "audio" and "video" default to true via serde — see
    // src/migration/protocol.rs:84-91. Omit unless overriding.
}))?;
```

Source of truth: `CreateMixer` variant in
[`protocol.rs:84-91`](../../src/migration/protocol.rs) +
crossfade test in [`lib.rs:283-307`](../../src/lib.rs).

### 3.2 `createsource` ×2

```rust
run_graph_command("createsource", json!({
    "id":  src_a_id,                            // "src-a-<uuid>"
    "uri": inputs.src_a.uri,                    // "srt://relay.example:9710?..."
}))?;

run_graph_command("createsource", json!({
    "id":  src_b_id,
    "uri": inputs.src_b.uri,
}))?;
```

Source of truth: `CreateSource` variant in
[`protocol.rs:55-63`](../../src/migration/protocol.rs).

> Latency / stream-id from `inputs.src_a.latency_ms` and
> `inputs.src_a.stream_id` must be inlined into the URI as query
> parameters (`?latency=2000&streamid=publish:my-key`) — the protocol
> takes a single `uri` string, not a structured options dict. PHASE-12's
> handler is the right place to concatenate them. If the SRT helper in
> `src/migration/sources/srt.rs` (or wherever the runtime parses the
> URI) does not honour `latency` / `streamid` query params,
> the runtime needs fixing in a separate phase — **not** PHASE-12.

### 3.3 `createdestination`

```rust
let publish_uri = if inputs.dst.stream_key.is_empty() {
    inputs.dst.uri.clone()
} else {
    format!("{}/{}", inputs.dst.uri, inputs.dst.stream_key)
};

run_graph_command("createdestination", json!({
    "id": dst_id,                                  // "dst-rtmp-<uuid>"
    "family": { "Rtmp": { "uri": publish_uri } },  // tagged variant
}))?;
```

Source of truth: `CreateDestination` variant in
[`protocol.rs:72-79`](../../src/migration/protocol.rs) +
`DestinationFamily::Rtmp { uri }` in
[`protocol.rs:148-150`](../../src/migration/protocol.rs).

> The `family` JSON shape is the externally-tagged form Serde produces
> by default for un-attributed enums:
> `{ "Rtmp": { "uri": "rtmp://…" } }`. **Not** `{ "rtmp": { … } }` —
> `DestinationFamily` does **not** carry `#[serde(rename_all =
> "lowercase")]` (the attribute is only on the outer `Command` enum).
> If the serde shape feels surprising, add a unit test that
> round-trips a `DestinationFamily::Rtmp { uri }` through `serde_json`
> before shipping — see the protocol-level tests near
> `src/migration/protocol.rs` end of file (search for `#[test]`).

### 3.4 `connect` × 3 (mixer → dst, src-A → mixer, src-B → mixer)

```rust
// Mixer → destination (mixer's combined output feeds the RTMP sink).
run_graph_command("connect", json!({
    "link_id": link_mixer_to_dst,                  // "link-mix-dst-<uuid>"
    "src_id":  mixer_id,
    "sink_id": dst_id,
    // audio and video default to true via serde; the mixer emits
    // both, the RTMP destination accepts both. Omit unless overriding.
}))?;

// Source A → mixer slot.
run_graph_command("connect", json!({
    "link_id": link_a,                             // "link-srcA-mix-<uuid>"
    "src_id":  src_a_id,
    "sink_id": mixer_id,
    "audio":   true,
    "video":   true,
    "config":  slot_config(&inputs.src_a, &inputs.canvas),
}))?;

// Source B → mixer slot.
run_graph_command("connect", json!({
    "link_id": link_b,
    "src_id":  src_b_id,
    "sink_id": mixer_id,
    "audio":   true,
    "video":   true,
    "config":  slot_config(&inputs.src_b, &inputs.canvas),
}))?;
```

where `slot_config` produces the connect-time config dict — exactly
the keys the crossfade test uses
([`lib.rs:354-362`](../../src/lib.rs)):

```rust
fn slot_config(src: &SrtSource, canvas: &MixerCanvas) -> serde_json::Value {
    json!({
        "video::zorder":        src.mix_zorder,
        "video::alpha":         src.mix_alpha,
        "video::width":         canvas.width,
        "video::height":        canvas.height,
        "video::sizing-policy": "keep-aspect-ratio",
        "audio::volume":        src.mix_volume,
    })
}
```

Source of truth: `Connect` variant in
[`protocol.rs:91-100`](../../src/migration/protocol.rs) +
crossfade reference in [`lib.rs:352-362`](../../src/lib.rs).

> The `audio::volume` key is the only entry **not** literally present
> in the crossfade test (which uses a video-only generator). If the
> migration runtime's mixer slot handler rejects `audio::volume`,
> drop the key and add a note to the per-source error path; the
> volume slider becomes UI-only in that case. Fix in
> `src/migration/mixers/...` lands as a separate phase, **not**
> PHASE-12.

### 3.5 `start` × 4

```rust
// Start downstream-first so the mixer doesn't block on a missing sink.
// Mirrors the crossfade test order in lib.rs:328-340 (dst first, then
// mixer, then sources).
run_graph_command("start", json!({ "id": dst_id }))?;
run_graph_command("start", json!({ "id": mixer_id }))?;
run_graph_command("start", json!({ "id": src_a_id }))?;
run_graph_command("start", json!({ "id": src_b_id }))?;
```

Source of truth: `Start` variant in
[`protocol.rs:101-105`](../../src/migration/protocol.rs).

> **`cue_time` / `end_time` are omitted.** The optional fields default
> to `None` via serde — the runtime starts immediately and runs
> open-ended.

### 3.6 Writeback to Bridge

After every successful command, the handler updates the matching
struct field on Bridge so the UI status indicators advance:

```rust
let _ = ui_weak.upgrade_in_event_loop(move |ui| {
    let bridge = ui.global::<Bridge>();

    let mut a = bridge.get_srt_source_a();
    a.slot_id = link_a.into();          // SharedString from String
    a.state   = MixerState::Running;
    bridge.set_srt_source_a(a);

    let mut b = bridge.get_srt_source_b();
    b.slot_id = link_b.into();
    b.state   = MixerState::Running;
    bridge.set_srt_source_b(b);

    let mut dst = bridge.get_rtmp_destination();
    dst.node_id = dst_id.into();
    dst.state   = MixerState::Running;
    bridge.set_rtmp_destination(dst);

    let mut canvas = bridge.get_mixer_canvas();
    canvas.node_id = mixer_id.into();
    canvas.state   = MixerState::Running;
    bridge.set_mixer_canvas(canvas);

    bridge.set_mixer_state(MixerState::Running);
    bridge.set_mixer_error_text("".into());
});
```

> **Slint-doc reference:** the Rust-side struct setter pattern
> (read–mutate–write) is
> [`globals.mdx`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx)
> §Rust tab — `app.global::<Logic>().set_the_value(42)`. Slint struct
> properties are passed by value across the FFI boundary, so the
> read–mutate–write triplet is the only correct pattern (you cannot
> mutate the live struct in place).

### 3.7 Failure-path teardown

If any of §3.1–§3.5 returns `Err(...)`, the handler must:

1. Walk back through the IDs it has created so far (mixer, src A, src
   B, dst, link A, link B, link mixer-dst) and issue `disconnect` /
   `remove` for each (best-effort — log errors, do **not** abort the
   cleanup loop).
2. Set `Bridge.mixer-state = MixerState::Error`.
3. Set `Bridge.mixer-error-text = <error message>`.
4. Also propagate the error into the offending sub-struct's
   `last-error` field so the inline error label renders.

This is exactly the pattern the crossfade test uses
([`lib.rs:386-425`](../../src/lib.rs) — see the cleanup block guarded
by `slot_source_created`, `destination_created`, `mixer_created`
booleans).

---

## 4. `Bridge.stop-mixer-cast()` handler

Same shape, reversed order — sources first, then mixer, then
destination:

```rust
let inputs = snapshot_inputs(&ui);

// Disconnect (best-effort; ignore errors for cleanup).
let _ = run_graph_command("disconnect",
    json!({ "link_id": inputs.src_a.slot_id }));
let _ = run_graph_command("disconnect",
    json!({ "link_id": inputs.src_b.slot_id }));
// (the mixer → dst link id is not stored on Bridge — the handler
// must remember it from the start sequence in §3.4; PHASE-12 can
// either stash it in a Rust-side `Arc<Mutex<Option<String>>>` or
// add a new `Bridge.mixer-link-id` `in` property.)
let _ = run_graph_command("disconnect",
    json!({ "link_id": mixer_to_dst_link_id }));

let _ = run_graph_command("remove",
    json!({ "id": inputs.src_a.slot_id /* actually src node id */ }));
let _ = run_graph_command("remove",
    json!({ "id": inputs.src_b.slot_id /* same */ }));
let _ = run_graph_command("remove",
    json!({ "id": inputs.canvas.node_id }));
let _ = run_graph_command("remove",
    json!({ "id": inputs.dst.node_id }));
```

> The comment "actually src node id" hints at a design issue:
> `SrtSource` carries one id field (`slot-id`) but the handler needs
> **two** ids per source — the `createsource` id and the `connect`
> `link_id`. PHASE-12 has two options:
>
> 1. Split `slot-id` into two fields (`src-id` + `link-id`) on the
>    `SrtSource` struct (a STEP-2 amendment).
> 2. Keep `slot-id` and store the `src-id` Rust-side in an
>    `Arc<Mutex<HashMap<&str, MixerNodeIds>>>` keyed by `"a"` / `"b"`.
>
> Option 2 is simpler and keeps STEP-2 stable; option 1 is more
> testable. PHASE-12 should pick option 2 unless a test surface
> appears.

---

## 5. `Bridge.apply-mixer-slot-config(slot_id, alpha, zorder, volume)`

```rust
ui.global::<Bridge>().on_apply_mixer_slot_config(move |slot_id, alpha, zorder, volume| {
    if slot_id.is_empty() {
        return; // Not yet bound to a live link — slider drag pre-Start.
    }
    let _ = run_graph_command("connect", json!({
        "link_id": slot_id.as_str(),
        // The mixer connect handler treats a re-`connect` with the
        // same link_id as "update the config dict for this slot."
        // src_id and sink_id must still be supplied (the protocol
        // enforces them) — read from a side table or from the
        // matching SrtSource struct.
        "src_id":  /* read from cache */,
        "sink_id": /* mixer_id from cache */,
        "audio":   true,
        "video":   true,
        "config": {
            "video::alpha":   alpha,
            "video::zorder":  zorder,
            "audio::volume":  volume,
        },
    }));
});
```

> **Assumption:** the migration runtime's mixer node accepts a
> re-`connect` with the same `link_id` as a config update. If it does
> not (e.g. it errors with "link_id already exists"), the handler
> must `disconnect` + `connect` instead, which causes a 1-2 frame
> blank on the mixer slot. PHASE-12 should write a test against the
> in-process runtime to pick the right path before shipping.

If `slot_id` corresponds to no known cached slot (e.g. the user
stopped the cast between drag start and drag end), the handler
silently returns.

---

## 6. `Bridge.apply-mixer-canvas(w, h, sr)`

```rust
ui.global::<Bridge>().on_apply_mixer_canvas(move |w, h, sr| {
    // Always stash on the Bridge struct (the slider already two-way-
    // bound, so this is redundant in the idle state — but harmless).
    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<Bridge>();
        let mut canvas = bridge.get_mixer_canvas();
        canvas.width       = w;
        canvas.height      = h;
        canvas.sample_rate = sr;
        bridge.set_mixer_canvas(canvas);
    });

    // Live-resize is not supported by the in-tree runtime today. If
    // the cast is running, surface a banner instead of issuing the
    // teardown + restart automatically.
    let state = /* read MixerState from a Rust-side mirror */;
    if state == MixerState::Running {
        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
            ui.global::<Bridge>().set_mixer_error_text(
                "Canvas changes apply on the next Start.".into());
        });
    }
});
```

> Live mixer-config mutation (resizing the canvas without tearing the
> graph down) is **not** in the migration runtime today. The
> `Command::Reschedule` variant
> ([`protocol.rs:106-110`](../../src/migration/protocol.rs)) only
> reschedules cue/end times. PHASE-12 ships the "apply on next Start"
> compromise; a follow-on phase can add a `ReconfigureMixer` variant
> later if live resize becomes important.

---

## 7. PHASE-12 readiness checklist (this step's deliverable)

For PHASE-12 to land the Rust handlers, the following must be true:

- [ ] Every JSON shape in §3 round-trips through `serde_json` against
      the in-process runtime (`run_graph_command("createmixer",
      …)` returns `{"result":"success"}`, not an error).
- [ ] The `DestinationFamily::Rtmp` variant's serde shape is verified
      with a one-line unit test:
      `assert_eq!(serde_json::to_value(DestinationFamily::Rtmp { uri:
      "x".into() }).unwrap(), json!({"Rtmp":{"uri":"x"}}));`
- [ ] The mixer's `connect` accepts `"audio::volume": <0..1 float>`
      as a slot config key. (If not, drop §3.4 `audio::volume` and
      surface a TODO.)
- [ ] The handler uses the same `std::thread::spawn` + `upgrade_in_event_loop`
      pattern as PHASE-9 STEP-2 — no synchronous Slint calls from
      the UI thread.
- [ ] A unit test in `src/lib.rs` (or `src/migration/runtime.rs`)
      smokes the full §3.1–§3.5 ladder against
      `try_handle_command_json` — same shape as the existing
      crossfade test, but using `LocalFile` (write to a temp dir) as
      the destination instead of `Rtmp` (which requires a network
      sink). RTMP smoke can come later behind an integration-test
      flag.
- [ ] When any of the four handlers fires, the resulting
      `Bridge.mixer-state` walk is observable via the existing
      `Bridge.test-status` field (PHASE-9 audit hook in
      `src/lib.rs:log_ui_test_status`) so the migration smoke tests
      can verify the writeback path.

---

## 8. Exit gate

- [ ] All §3–§6 JSON shapes have explicit `protocol.rs` line citations.
- [ ] All §3.6 / §4 writebacks are documented to use
      read–mutate–write on Bridge struct properties (the only valid
      pattern across the Slint FFI).
- [ ] The "assumptions worth verifying before PHASE-12 ships" list in
      §7 has been distilled into an issue or a draft phase doc.
- [ ] PHASE-11 closes here — STEP-1 through STEP-8 shipped, STEP-9
      handed to PHASE-12.

End of MVP-PHASE-11.
