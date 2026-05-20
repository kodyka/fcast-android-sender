# 11 — Back-stack & navigation: replace direct `active-panel = …` writes

## Goal

Every place in the UI that "opens" or "closes" a panel goes through a
typed helper (`PanelBridge.push(p)`, `PanelBridge.pop()`,
`PanelBridge.replace(p)`). The helper maintains a back-stack so the
Android back-key consistently lands on the **previous** panel instead
of always closing to `Panel.none`. Cross-panel links (e.g. "Settings ▸
Media backend ▸ Save → return to Settings") work without each panel
hard-coding which panel to open on close.

## Findings

### F14 — `active-panel` is written from 50+ sites with no back-stack

`grep -rn 'Bridge.active-panel =' ui | wc -l` → **58 hits** across 25
pages and components.

Pattern A (most common): "close" means "go to none":

```slint
TextButton {
    label: @tr("close-panel-button" => "Done");
    clicked => { Bridge.active-panel = Panel.none; }
}
```

Pattern B: "go to a sibling" (close one panel and open another):

```slint
// macro_edit_page.slint
PrimaryButton {
    clicked => {
        Bridge.save-macro(…);
        Bridge.active-panel = Panel.macros;
    }
}
```

Pattern C: "open from a row tap":

```slint
TouchArea {
    clicked => {
        Bridge.selected-history-id = entry.id;
        Bridge.active-panel = Panel.cast-history-detail;
    }
}
```

The back-key handler in `main.slint:103–118`:

```slint
back-key-scope := FocusScope {
    key-pressed(event) => {
        if (event.text == Key.Escape) {
            if (Bridge.active-panel != Panel.none) {
                Bridge.active-panel = Panel.none;
                return accept;
            }
            return reject;
        }
        return reject;
    }
}
```

Effects:

- "Settings ▸ Media backend" + back → goes to **the connect/casting
  page** (the panel underneath), not back to Settings. Users have to
  re-open Settings every time.
- "Macros ▸ Macro edit" + cancel → goes to Macros (handled
  hard-coded in `macro_edit_page.slint`).
- No notion of "deep link → return to whence I came" — the destination
  page hard-codes its return.

## Slint docs reference

- [`functions-and-callbacks.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx)
  — `public function name(args) { … }` on globals; callable from any
  `.slint` file that imports the global.
- [`properties.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/properties.mdx)
  — `private property <…>` for state that should not be visible to
  callers; `in-out property <[…]>` for arrays you mutate by re-assigning.
- Slint 1.16+ supports array slicing on properties (`stack[1..]`);
  on 1.15.1, use a Rust-side helper. The 1.15-compatible Slint-only
  shape below uses a pure helper function.

## Before — direct writes everywhere

```slint
// connect_page.slint — quick-action handler
QuickActionButton {
    invoked(id) => {
        if (id == "settings") { Bridge.active-panel = Panel.settings; }
        // …
    }
}

// receiver_context_menu.slint
rename-clicked => {
    Bridge.active-panel = Panel.receiver-rename;
}
```

## After — `PanelBridge` push/pop helpers

```slint
// ui/state/panels.slint   (after step 02)
export global PanelBridge {
    in-out property <Panel> active: Panel.none;

    // Back-stack — innermost panel at index 0.
    // Slint 1.15: this stays Slint-private because we can't slice
    // arrays without 1.16+. The helpers below mutate it as a whole.
    private property <[Panel]> stack: [];

    public function push(p: Panel) {
        // Push the *currently visible* panel onto the stack, then
        // activate `p`. If `p == active`, no-op.
        if PanelBridge.active == p { return; }
        if PanelBridge.active != Panel.none {
            PanelBridge.stack = prepend(PanelBridge.stack, PanelBridge.active);
        }
        PanelBridge.active = p;
    }

    public function pop() {
        if PanelBridge.stack.length == 0 {
            PanelBridge.active = Panel.none;
            return;
        }
        PanelBridge.active = PanelBridge.stack[0];
        PanelBridge.stack = drop-first(PanelBridge.stack);
    }

    public function replace(p: Panel) {
        // Like push, but does NOT save the current panel — used when
        // a "close" should not be reachable via back-key.
        PanelBridge.active = p;
    }

    public function close-all() {
        PanelBridge.active = Panel.none;
        PanelBridge.stack  = [];
    }

    // ── 1.15-compatible array helpers ───────────────────────────
    pure function prepend(xs: [Panel], x: Panel) -> [Panel] {
        // Slint 1.15 has no spread in array literals, no array.push,
        // and no slice operator on arrays. The portable trick is to
        // hand the mutation off to Rust via a callback. If you don't
        // want a Rust hop, hard-cap the stack depth at 4 and use a
        // tuple struct of 4 Panel-or-none slots. Pseudo-code below
        // assumes the Rust-callback route.
        callback prepend-stack(x);   // Rust appends and returns new
        return xs;
    }
    pure function drop-first(xs: [Panel]) -> [Panel] {
        callback drop-first-stack();
        return xs;
    }
}
```

> The "stack as array" implementation has two practical options on
> Slint 1.15.1:
>
> 1. **Rust-owned back-stack.** `PanelBridge.stack` is `in`-direction;
>    Rust hands Slint a fresh `[Panel]` each time. Slint
>    `push`/`pop` callbacks delegate to Rust. This is the cleanest
>    fit with how the rest of the Bridge already works.
> 2. **Fixed-depth tuple in Slint.** `struct PanelStack4 { a, b, c, d:
>    Panel }` and a depth count. Verbose; only viable if you commit to
>    "max 4 panels deep" (probably enough for this app).
>
> The post-step-02 design recommends **option 1** because it keeps the
> Slint side declarative and lets Rust persist the stack across
> rotations.

### Rust-owned back-stack — full shape

```rust
// senders/android/src/lib.rs
use std::cell::RefCell;
use std::rc::Rc;
use slint::{Model, ModelRc, VecModel};

struct PanelStack(RefCell<Vec<Panel>>);

impl PanelStack {
    fn new() -> Self { Self(RefCell::new(Vec::new())) }

    fn push(&self, current: Panel) {
        if current != Panel::None {
            self.0.borrow_mut().insert(0, current);
        }
    }
    fn pop(&self) -> Panel {
        self.0.borrow_mut().pop().unwrap_or(Panel::None)
    }
    fn as_model(&self) -> ModelRc<Panel> {
        ModelRc::new(VecModel::from(self.0.borrow().clone()))
    }
}

// In bootstrap:
let stack = Rc::new(PanelStack::new());

let panel_bridge = ui.global::<PanelBridge>();
panel_bridge.on_push({
    let stack = stack.clone();
    let ui_weak = ui.as_weak();
    move |p: Panel| {
        let Some(ui) = ui_weak.upgrade() else { return };
        let pb = ui.global::<PanelBridge>();
        let current = pb.get_active();
        if current == p { return; }
        stack.push(current);
        pb.set_active(p);
        pb.set_stack(stack.as_model());
    }
});

panel_bridge.on_pop({
    let stack = stack.clone();
    let ui_weak = ui.as_weak();
    move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let pb = ui.global::<PanelBridge>();
        pb.set_active(stack.pop());
        pb.set_stack(stack.as_model());
    }
});

panel_bridge.on_replace({
    let ui_weak = ui.as_weak();
    move |p: Panel| {
        if let Some(ui) = ui_weak.upgrade() {
            ui.global::<PanelBridge>().set_active(p);
        }
    }
});

panel_bridge.on_close_all({
    let stack = stack.clone();
    let ui_weak = ui.as_weak();
    move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        stack.0.borrow_mut().clear();
        let pb = ui.global::<PanelBridge>();
        pb.set_active(Panel::None);
        pb.set_stack(stack.as_model());
    }
});
```

…and `PanelBridge` declares the four callbacks on the Slint side:

```slint
export global PanelBridge {
    in-out property <Panel>   active: Panel.none;
    in property      <[Panel]> stack: [];

    callback push(p: Panel);
    callback pop();
    callback replace(p: Panel);
    callback close-all();
}
```

## Before — `main.slint:103–118` back-key handler

```slint
back-key-scope := FocusScope {
    key-pressed(event) => {
        if (event.text == Key.Escape) {
            if (Bridge.active-panel != Panel.none) {
                Bridge.active-panel = Panel.none;
                return accept;
            }
            return reject;
        }
        return reject;
    }
}
```

## After — back-key pops the stack

```slint
back-key-scope := FocusScope {
    key-pressed(event) => {
        if event.text == Key.Escape {
            if PanelBridge.active != Panel.none {
                PanelBridge.pop();
                return accept;
            }
            return reject;
        }
        return reject;
    }
}
```

## Call-site migration

### Pattern A — "close" buttons

```slint
// Before
TextButton {
    label: @tr("close-panel-button" => "Done");
    clicked => { Bridge.active-panel = Panel.none; }
}
// After
TextButton {
    label: @tr("close-panel-button" => "Done");
    clicked => { PanelBridge.pop(); }
}
```

### Pattern B — "close + open sibling" (macro edit → macros list)

```slint
// Before
PrimaryButton {
    clicked => {
        Bridge.save-macro(…);
        Bridge.active-panel = Panel.macros;
    }
}
// After
PrimaryButton {
    clicked => {
        Macros.save(…);
        PanelBridge.pop();         // pops back to whichever opened us
    }
}
```

### Pattern C — "open from a row tap"

```slint
// Before
TouchArea {
    clicked => {
        Bridge.selected-history-id = entry.id;
        Bridge.active-panel = Panel.cast-history-detail;
    }
}
// After
TouchArea {
    clicked => {
        History.open-detail(entry.id);
        PanelBridge.push(Panel.cast-history-detail);
    }
}
```

### Pattern D — Settings ▸ Media backend (deep open)

```slint
// In SettingsRow that opens the Media backend page:
SettingsValueRow {
    title: @tr("Media backend");
    value: MediaBackend.kind == MediaBackendKind.migration
            ? @tr("media-backend-engine-migration", "Migration (in-process)")
            : @tr("media-backend-engine-gstpop",     "gst-pop (WebSocket)");
    clicked => { PanelBridge.push(Panel.media-backend); }
}
```

When the user taps "Done" inside `MediaBackendPage`, `PanelBridge.pop()`
returns them to `Panel.settings` automatically — no hard-coded routing.

## A note on "replace" vs "push"

`PanelBridge.replace(p)` does not save the current panel — use it when
the destination shouldn't be returnable via back (e.g. after sign-out
that closes Settings and goes to Connect, the user should not be able
to back-key into the now-stale Settings).

Example:

```slint
// backup_reset_page.slint — Reset all data
DestructiveButton {
    label: @tr("Reset everything");
    clicked => {
        Bridge.reset-everything();
        PanelBridge.close-all();     // no back to anything
    }
}
```

## Cross-panel deep-link from Rust

When a push notification or share intent wants to open
`Panel.cast-history-detail` with a specific entry id, Rust should:

```rust
// senders/android/src/lib.rs
fn open_history_detail(ui: &MainWindow, entry_id: &str) {
    let history = ui.global::<History>();
    history.invoke_open_detail(entry_id.into());

    let panel = ui.global::<PanelBridge>();
    panel.invoke_close_all();                      // reset stack
    panel.invoke_push(Panel::SettingsCastHistory); // seed parent
    panel.invoke_push(Panel::SettingsCastHistoryDetail);
}
```

A back from the detail page lands on the cast-history list, a second
back lands on `Panel.none` (the connect page) — which is what users
expect.

## Migration

1. Add `PanelBridge` with `active`, `stack`, and the four callbacks.
2. Wire Rust to the four callbacks.
3. Replace **every** `Bridge.active-panel = Panel.X` write with one of
   `PanelBridge.push(Panel.X)`, `PanelBridge.replace(Panel.X)`, or
   `PanelBridge.pop()`. Decide per call-site:
   - "Open a child panel" → `push`
   - "Close this panel, go back to where I came from" → `pop`
   - "Replace this panel with a sibling (e.g. Cancel goes to a sibling
     list)" → `replace`
   - "Tear down all panels and go home" → `close-all`
4. Update the back-key handler in `main.slint` to call `pop`.
5. Spot-check deep flows: Settings ▸ Media backend → Done → lands on
   Settings; Macros ▸ Macro edit → Cancel → lands on Macros; Cast
   history ▸ Detail → Done → lands on Cast history.

### Per-file checklist (top sites — there are 50+ total)

| File                                       | Replacements                                              |
| ------------------------------------------ | --------------------------------------------------------- |
| `ui/main.slint`                            | back-key → `PanelBridge.pop()`                            |
| `ui/components/control_bar.slint`          | quick-action `id ==` arms → `PanelBridge.push(…)`         |
| `ui/components/receiver_context_menu.slint`| `rename-clicked => PanelBridge.push(Panel.receiver-rename)` |
| `ui/pages/connect_page.slint`              | scrim-tap → `PanelBridge.pop()`                           |
| `ui/pages/media_backend_page.slint`        | Done → `PanelBridge.pop()`; Save → `PanelBridge.pop()` after `MediaBackend.apply()` |
| `ui/pages/macro_edit_page.slint`           | Cancel/Save → `PanelBridge.pop()`                         |
| `ui/pages/macros_page.slint`               | Tap row → `PanelBridge.push(Panel.macro-edit)`             |
| `ui/pages/cast_history_page.slint`         | Tap entry → `PanelBridge.push(Panel.cast-history-detail)`  |
| `ui/pages/cast_history_detail_page.slint`  | Done → `PanelBridge.pop()`                                |
| `ui/pages/pairing_page.slint`              | `close =>` → `PanelBridge.pop()`                          |
| `ui/pages/receiver_rename_page.slint`      | `save`/`cancel` → `PanelBridge.pop()`                     |
| `ui/pages/bitrate_presets_page.slint`      | Tap preset → `push(Panel.bitrate-preset-edit)`             |
| `ui/pages/settings_page.slint`             | All `clicked` rows → `push(...)`                           |

## Out of scope

- Animated panel transitions on push/pop. Slint can do it (`states`
  with `in { animate x { … } }`); add as a follow-up if desired.
- Multiple-window scenarios (the Android sender is single-window).
- Persisting the back-stack across process death. The Rust-side stack
  is a normal `Vec`; bundle it into your existing session-state
  persistence if needed.

## Acceptance

- [ ] `git grep -nE 'Bridge\.active-panel =' ui/` returns **0** hits
      after the migration. (The PanelBridge global itself writes
      `PanelBridge.active = …`, which is fine.)
- [ ] Back from "Settings ▸ Media backend" lands on Settings.
- [ ] Back from "Settings ▸ Macros ▸ Macro edit" lands on Macros.
- [ ] Back from "Connect ▸ Receiver context menu ▸ Rename" lands on
      Connect.
- [ ] `PanelBridge.close-all()` from Rust after sign-out / reset works
      end-to-end.
