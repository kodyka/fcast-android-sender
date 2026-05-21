# UI testing tutorial — Slint headless tests for the FCast Android sender

> **Audience:** contributors who want to add a new `.slint` UI test, or wire one
> up in GitHub Actions, without having to reverse-engineer the existing setup.

This walkthrough is grounded in the files that already exist in this repo:
[`tests/ui_snapshots.rs`](../tests/ui_snapshots.rs),
[`build.rs`](../build.rs), and
[`.github/workflows/slint-viewer-smoke.yml`](../.github/workflows/slint-viewer-smoke.yml).

By the end you will know:

1. The two test styles in use here — **globals-only snapshots** and
   **behavioural element-walk tests** — and when to reach for each.
2. How to run them locally inside `nix develop`.
3. How to add a new test scope, including the gotchas that have already
   cost commits to fix.
4. How the CI workflow validates `.slint` files independently of the Rust
   tests, and where to plug in a `cargo test` job if you want one.

---

## 0. What these tests do and don't catch

Slint's headless backend (`i-slint-backend-testing`) lets us instantiate a
`MainWindow` in a unit test without a real display, drive its globals and
callbacks, and walk its accessibility tree. That covers:

- **Navigation invariants.** Push/pop of `PanelBridge` actually returns to the
  right panel after a back-button press.
- **Geometry contracts.** `SafeArea.top` clamps raw insets to `min-top`;
  `SafeArea.bottom` does not have a 65 px floor.
- **Behavioural flows.** Clicking the "Settings" tile on the home screen
  actually opens `Panel::Settings`, end-to-end through the repeater, the
  `accessible-action-default` handler, and `PanelBridge.push`.

It does **not** catch:

- Visual regressions (no rendering happens).
- Real Android-runtime issues — JNI, lifecycle, GStreamer.
- Slint compilation breaks in files that `ui/main.slint` does not transitively
  import. That gap is covered by
  [`slint-viewer-smoke`](../.github/workflows/slint-viewer-smoke.yml).

If your change touches the visual layer or the Android runtime, this test
suite is a sanity check — not a substitute for testing on a device.

---

## 1. Prerequisites

| Tool | Version | Provided by |
|------|---------|-------------|
| Rust | matches workspace | `nix develop` |
| Slint | `1.16.0` | `Cargo.toml` |
| `i-slint-backend-testing` | `1.16.0` | `Cargo.toml` (dev-dependency) |
| `slint-viewer` | `1.16.0` | `nix-shell -p slint-viewer` |

The repo's `Cargo.toml` already declares the testing backend:

```toml
[dev-dependencies]
i-slint-backend-testing = "1.16.0"
```

The version **must match** `slint`. A mismatch produces opaque panics at
window creation time.

---

## 2. The `build.rs` debug-info gate

`i_slint_backend_testing::ElementHandle::find_by_*` and `accessible_role()`
need Slint debug info baked into the generated Rust code. Without it,
`find_by_accessible_label` silently returns an empty iterator and you'll see
this warning at test start:

```
The use of the ElementHandle API requires the presence of debug info in
Slint compiler generated code. Set the `SLINT_EMIT_DEBUG_INFO=1`
environment variable at application build time …
```

The repo wires this in [`build.rs`](../build.rs):

```rust
let target = env::var("TARGET").unwrap();
let mut config = slint_build::CompilerConfiguration::new();
if !target.contains("android") {
    // Host builds (= tests) need debug info for ElementHandle.
    // Android builds skip it to keep the .so smaller.
    config = config.with_debug_info(true);
}
slint_build::compile_with_config("ui/main.slint", config).unwrap();
```

`with_debug_info(self, enable: bool) -> Self` is a public method on
`slint_build::CompilerConfiguration` in the `slint-build = "1.16.0"` and
`1.16.1` releases (see `slint-build/lib.rs:223` in the crate source). The
equivalent escape hatch is the `SLINT_EMIT_DEBUG_INFO=1` env var at build
time — useful if you ever invoke the compiler outside `build.rs`.

You do **not** need to do anything for this — it is on for host targets by
default. If you ever see the warning above, the gate has regressed.

---

## 3. Two test styles

### Style A — globals-only snapshot test

Use when the assertion only touches a Slint **global** (`PanelBridge`,
`SafeArea`, `Bridge`, …). No element walking, no accessibility queries.

Pattern, copied verbatim from `tests/ui_snapshots.rs`:

```rust
// 1. panel_bridge_push_pop_roundtrip
{
    let ui = MainWindow::new().expect("MainWindow::new");
    wire_panel_bridge(&ui);
    let pb = ui.global::<PanelBridge>();

    assert_eq!(pb.get_active(), Panel::None, "initial state is Panel::None");

    pb.invoke_push(Panel::Settings);
    assert_eq!(pb.get_active(), Panel::Settings);

    pb.invoke_push(Panel::Audio);
    assert_eq!(pb.get_active(), Panel::Audio);

    pb.invoke_pop();
    assert_eq!(pb.get_active(), Panel::Settings, "pop returns to Settings");

    pb.invoke_pop();
    assert_eq!(pb.get_active(), Panel::None, "pop to root returns Panel::None");
}
```

`wire_panel_bridge(&ui)` is a helper in the same file that mirrors the Rust
PanelStack semantics from `src/lib.rs` — `push` stacks the previous active,
`pop` removes the front entry, `replace` swaps without growing the stack,
`close-all` clears it. Future tests should reuse it rather than duplicating
the wiring.

### Style B — behavioural test (ElementHandle)

Use when the assertion is "find the UI element a user would interact with
and trigger it." Example: the
[Settings-button → Settings-panel scope added in PR #14](../tests/ui_snapshots.rs):

```rust
// 7. home_screen_settings_button_opens_settings_panel
{
    let ui = MainWindow::new().expect("MainWindow::new");
    wire_panel_bridge(&ui);
    let pb = ui.global::<PanelBridge>();
    let bridge = ui.global::<Bridge>();

    // Seed Bridge.quick-actions with the OpenSettings entry that
    // `default_quick_actions()` ships in production (src/lib.rs).
    let actions = std::rc::Rc::new(slint::VecModel::from(vec![QuickAction {
        kind: QuickActionKind::OpenSettings,
        title: "Settings".into(),
        macro_id: "".into(),
        custom_id: "".into(),
        enabled: true,
        active: false,
    }]));
    bridge.set_quick_actions(actions.into());

    // Filter by AccessibleRole::Button — the inner `Text { text: "Settings" }`
    // inside QuickActionButton also auto-derives accessible-label "Settings"
    // (role=text), so find_by_accessible_label returns both. See §8 for the
    // detailed explanation.
    let buttons: Vec<_> = ElementHandle::find_by_accessible_label(&ui, "Settings")
        .filter(|el| el.accessible_role() == Some(AccessibleRole::Button))
        .collect();
    assert_eq!(buttons.len(), 1);

    buttons[0].invoke_accessible_default_action();
    assert_eq!(pb.get_active(), Panel::Settings);
}
```

Imports needed at the top of the test file:

```rust
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::ComponentHandle;
```

**Action menu** — pick the simulation that matches what you want to assert:

| API | Style | Use when |
|-----|-------|----------|
| `el.invoke_accessible_default_action()` | sync, no event loop | A click / keyboard activation that fires the registered `accessible-action-default`. Fastest, used in this repo. |
| `i_slint_backend_testing::send_mouse_click(&ui, x, y)` | sync | You need a click at specific coordinates (e.g. testing hit-test boundaries). Get `x` / `y` from `el.absolute_position()` + `el.size()`. |
| `el.single_click(PointerEventButton::Left).await` | async | You need a realistic move → press → wait → release sequence. Requires either (a) `init_integration_test_with_system_time()` + `slint::spawn_local` + `slint::run_event_loop()`, or (b) enabling the **`internal`** feature on `i-slint-backend-testing` to expose `block_on`. This repo does neither, so this API is not used here. |
| `i_slint_backend_testing::send_keyboard_string_sequence(&ui, &Key::Return.into())` | sync | Keyboard activation; pair with `instance.invoke_focus_<name>()` to focus first. |

For finding elements:

| API | Returns | Best for |
|-----|---------|----------|
| `ElementHandle::find_by_accessible_label(&ui, "X")` | iterator | User-visible buttons / rows. Filter by `accessible_role()` if the label is not unique. |
| `ElementHandle::find_by_element_id(&ui, "Component::ident")` | iterator | A specific named element (`ident := …`) in a known component. |

---

## 4. Conventions in this repo

These have been learned the hard way; keep new tests consistent.

### One `#[test]` function, multiple scopes

`tests/ui_snapshots.rs` declares a single `#[test] fn ui_snapshots_all()` and
runs every scenario as a `{ … }` block inside it. **Do not** add a second
`#[test]` function.

Reason: the *integration-test* backends (`init_integration_test_with_mock_time`
and `init_integration_test_with_system_time`) can only be initialised once per
process, and `cargo test` runs each `#[test]` on its own thread. A second
`#[test]` function creating a `MainWindow` from another thread panics with
"backend already initialised".

(`init_no_event_loop()` is the exception — its docs say each test thread can
have its own backend. But it cannot drive timers or `slint::invoke_from_event_loop`,
so this repo doesn't use it.)

To add a new test, append a new scope to the existing function:

```rust
#[test]
fn ui_snapshots_all() {
    init_headless();

    // … existing scopes …

    // 8. my_new_test
    {
        let ui = MainWindow::new().expect("MainWindow::new");
        // … your asserts …
    }
}
```

### Backend initialisation

The file initialises the backend once:

```rust
fn init_headless() {
    i_slint_backend_testing::init_integration_test_with_mock_time();
}
```

Three init modes exist; pick by scenario:

| Init function | When to use |
|--------------|-------------|
| `init_no_event_loop()` | Pure synchronous tests, no animations, no timer callbacks. Cheapest. Per-thread backends are allowed (one-`#[test]` rule doesn't apply). |
| `init_integration_test_with_mock_time()` | Need a controllable clock for `animate`, `Timer`, etc. **This repo's default.** Once per process. |
| `init_integration_test_with_system_time()` | Async tests with `single_click().await` driven by `slint::spawn_local` + `slint::run_event_loop()`. Once per process. |

### Globals access

```rust
let pb     = ui.global::<PanelBridge>();
let sa     = ui.global::<SafeArea>();
let bridge = ui.global::<Bridge>();
```

These resolve at runtime because `ui/main.slint` re-exports them so
`slint_build` generates the bindings. If `ui.global::<Foo>()` fails to
compile, the global is missing a `export { Foo }` line in `ui/main.slint`.

---

## 5. Running tests locally

### Inside `nix develop` (recommended)

```sh
nix develop --command cargo test --test ui_snapshots
```

Expected output:

```
running 1 test
test ui_snapshots_all ... ok

test result: ok. 1 passed; 0 failed; …
```

The first run compiles the host toolchain (slow). Subsequent runs are fast
because `nix develop` caches the dev shell.

### Outside nix

If you have Rust + Slint dev deps installed system-wide, plain `cargo test
--test ui_snapshots` works. The repo standardises on the nix shell so
versions match CI.

### Static lint pass

The accompanying static audit:

```sh
nix develop --command ci/ui-validate.sh --no-build
```

This runs the same rules CI applies (no raw hex, no `Bridge.active-panel =`,
etc.) without rebuilding the Android target.

### Previewing a `.slint` file with `slint-viewer`

For visual iteration:

```sh
nix-shell -p slint-viewer --run "slint-viewer ui/main.slint --auto-reload"
```

`scripts/check-slint-viewer.sh` enforces that any locally-installed
`slint-viewer` matches the workspace pin.

---

## 6. Adding a new test step by step

Suppose you want to test that the "Debug" quick-action button opens
`Panel::Debug`.

### Step 1 — read the production wiring

```sh
grep -n 'Panel.debug' ui/components/control_bar.slint
```

You'll find `PanelBridge.push(Panel.debug)` next to
`QuickActionKind.open-debug`, and `title: "Debug"` in
`src/lib.rs:default_quick_actions()`.

### Step 2 — append a new scope

In `tests/ui_snapshots.rs`, after the last existing scope:

```rust
// 8. home_screen_debug_button_opens_debug_panel
{
    let ui = MainWindow::new().expect("MainWindow::new");
    wire_panel_bridge(&ui);
    let pb = ui.global::<PanelBridge>();
    let bridge = ui.global::<Bridge>();

    let actions = std::rc::Rc::new(slint::VecModel::from(vec![QuickAction {
        kind: QuickActionKind::OpenDebug,
        title: "Debug".into(),
        macro_id: "".into(),
        custom_id: "".into(),
        enabled: true,
        active: false,
    }]));
    bridge.set_quick_actions(actions.into());

    let buttons: Vec<_> = ElementHandle::find_by_accessible_label(&ui, "Debug")
        .filter(|el| el.accessible_role() == Some(AccessibleRole::Button))
        .collect();
    assert_eq!(buttons.len(), 1);

    buttons[0].invoke_accessible_default_action();
    assert_eq!(pb.get_active(), Panel::Debug);
}
```

### Step 3 — run locally

```sh
nix develop --command cargo test --test ui_snapshots
```

If `find_by_accessible_label` returns more or fewer than 1 hit, see
[§8 Troubleshooting](#8-troubleshooting).

### Step 4 — commit and push

```sh
git add tests/ui_snapshots.rs
git commit -m "test(ui): Debug quick-action opens Debug panel"
git push -u origin feat/debug-button-test
gh pr create
```

CI will run `ui-lint`, `ui-validate`, and `slint-viewer-smoke`. There is no
dedicated `cargo test` job for `ui_snapshots` yet — see
[§7](#7-github-actions-integration) for how to add one.

---

## 7. GitHub Actions integration

### What runs today

Three UI-relevant jobs are wired up:

| Workflow | File | Triggers on | Purpose |
|----------|------|-------------|---------|
| `ui-lint` | [`ui-lint.yml`](../.github/workflows/ui-lint.yml) | PRs touching `ui/**` | Forbid raw hex, font-size px, direct `Bridge.active-panel` writes, etc. |
| `slint-viewer-smoke` | [`slint-viewer-smoke.yml`](../.github/workflows/slint-viewer-smoke.yml) | PRs touching `ui/**`, `Cargo.toml`, or itself | Download official `slint-viewer-linux` binary matching the Slint pin, run it under `xvfb-run` against `ui/main.slint`, treat exit 255 as compile fail. |
| `ui-validate` | [`gstpop-smoke.yml`](../.github/workflows/gstpop-smoke.yml) | PRs touching repo root | Runs `ci/ui-validate.sh --no-build`. |

None of these run `cargo test --test ui_snapshots` today.

### Adding a `cargo test` job for `ui_snapshots`

Create `.github/workflows/ui-tests.yml`:

```yaml
name: ui-tests

on:
  pull_request:
    paths:
      - 'ui/**'
      - 'tests/**'
      - 'src/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - 'build.rs'
      - '.github/workflows/ui-tests.yml'

jobs:
  ui-tests:
    name: cargo test --test ui_snapshots
    runs-on: ubuntu-latest
    timeout-minutes: 15

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        run: |
          rustup show
          rustup component add rustfmt clippy

      - name: Cache cargo registry + target
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: cargo-${{ runner.os }}-${{ hashFiles('Cargo.lock') }}

      - name: Run snapshot tests
        run: cargo test --test ui_snapshots
```

Notes:

- No `xvfb` is required — the testing backend is headless by design.
- No Android NDK / GStreamer is required because the test target is the host
  Rust target (`cfg(not(target_os = "android"))`), and `build.rs` only loads
  Android-specific deps when `TARGET` contains "android".
- The first run compiles `slint-build` and the test binary from scratch
  (~3–5 min). The cache step brings subsequent runs to <1 min.

### Inspecting test failures in CI

`cargo test` prints panic messages with file:line. If the failure is in a
`find_by_accessible_label` count, the assertion message identifies the
scope; cross-reference with this file to find the section number.

---

## 8. Troubleshooting

### `find_by_accessible_label` returns 0

- **Cause:** Slint debug info not emitted.
- **Fix:** check `build.rs` — `with_debug_info(true)` must be set for the
  current target (see §2). The warning
  `The use of the ElementHandle API requires the presence of debug info …`
  is printed at test startup if it's missing.

### `find_by_accessible_label` returns 2 or more when you expected 1

- **Cause:** Multiple **always-instantiated** elements expose the same
  accessible-label. Conditional `if cond: Component {}` branches are *not*
  instantiated when `cond` is false — the walker in `search_api.rs` skips
  invisible items — so they are not the source of extra matches. The usual
  culprit is one of:
  - A `Rectangle { accessible-role: button; accessible-label: "X" }` *and*
    a `Text { text: "X" }` inside it. Slint auto-derives accessible-label
    from a `Text`'s content, so both elements show up (roles `button` and
    `text`).
  - A repeater (`for entry in model:`) that renders multiple tiles with the
    same label.
  - A `PanelHeader { title: "X" }` that lives outside a panel gate — i.e.
    in always-visible chrome, not inside a `PanelHost` conditional.
- **Fix:** narrow by role:
  ```rust
  .filter(|el| el.accessible_role() == Some(AccessibleRole::Button))
  ```
  Or by element id:
  ```rust
  ElementHandle::find_by_element_id(&ui, "QuickActionButton::ta")
  ```

### Test panics with `backend already initialised`

- **Cause:** A second `#[test]` function tried to create a `MainWindow`.
- **Fix:** add your scope to `ui_snapshots_all`, not as a new `#[test]` (see
  §4 "One `#[test]` function").

### `ui.global::<Foo>()` won't compile

- **Cause:** `Foo` is defined in a `.slint` file but not re-exported through
  `ui/main.slint`.
- **Fix:** add `import { Foo } from "..."` then `export { Foo }` in
  `ui/main.slint`. Two statements — `import` ends with `;`, bare `export {…}`
  does not.

### `slint-viewer-smoke` fails with `libxxx.so: cannot open shared object`

- **Cause:** `slint-viewer-linux` is dynamically linked; CI runner is missing
  a runtime library.
- **Fix:** add the package to `Install xvfb and Slint runtime deps` in
  `.github/workflows/slint-viewer-smoke.yml`. The workflow already includes
  `libxkbcommon-x11-0`, `libinput10`, `libudev1`, `libwayland-*`, etc.; if a
  new lib is needed, the diagnostic `ldd | grep "not found"` step will name it.

### `cargo test` succeeds locally but fails in CI

Most common cause is the Cargo cache disagreeing with the source. Bust the
cache by changing the `key:` line in the workflow (e.g. bump a version
suffix), or temporarily remove the cache step.

---

## 9. Reading list

In this repo:

- [`tests/ui_snapshots.rs`](../tests/ui_snapshots.rs) — the canonical test file. Every example here is a real scope inside it.
- [`docs/ui-slint-best-practices/14-testing-and-validation.md`](./ui-slint-best-practices/14-testing-and-validation.md) — the migration step that added these tests; design rationale.
- [`scripts/check-slint-viewer.sh`](../scripts/check-slint-viewer.sh) — version gating for the local `slint-viewer`.

Upstream Slint:

- [`internal/backends/testing/README.md`](https://github.com/slint-ui/slint/blob/v1.16.0/internal/backends/testing/README.md) — `ElementHandle` reference and patterns.
- [`tests/cases/widgets/button.slint`](https://github.com/slint-ui/slint/blob/v1.16.0/tests/cases/widgets/button.slint) — full real-world example combining mouse, keyboard, and accessibility actions.
- [`tools/viewer/README.md`](https://github.com/slint-ui/slint/blob/v1.16.0/tools/viewer/README.md) — `slint-viewer` CLI, exit codes, `--save-data` / `--load-data` for shell-driven tests.
