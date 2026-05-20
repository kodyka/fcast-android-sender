# 14 — Testing & validation: `slint-viewer`, CI lint, snapshot, AT

## Goal

Make the UI changes from the previous steps **verifiable** without
booting the Android app. Add:

1. A `slint-viewer` workflow that loads each page in isolation.
2. A CI lint that catches the regressions this guide warns about
   (raw colour literals, `value == @tr(...)` ComboBox parsing,
   `Bridge.active-panel =` direct writes).
3. Snapshot tests via `i-slint-backend-testing` to detect pixel
   regressions on key panels.
4. Accessibility tree dumps so the a11y wiring from step 05 doesn't
   regress.

## Current state

- CI workflow: `ui-validate` runs `cargo check -p android-sender`
  which transitively compiles the Slint sources via `slint-build`. If
  a Slint file is syntactically broken or references a missing
  property, CI fails. **That's it.**
- No snapshot tests.
- No accessibility tree dumps.
- No `slint-viewer` documentation; the project doesn't ship one
  pre-configured.

`grep -rn 'slint-viewer\|i-slint-backend-testing' . | head` confirms
neither is referenced today.

## Slint docs reference

- [`testing.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/testing.mdx)
  — `i-slint-backend-testing` is the official headless backend; it
  yields an `accessible-tree` snapshot and supports programmatic
  click-and-type.
- [`slint-viewer`](../../draft/slint-ui/docs/astro/src/content/docs/quickstart/cli.mdx)
  — `cargo install slint-viewer` then `slint-viewer ui/main.slint`
  hot-reloads. Use `--component <Name>` to mount a sub-component.
- [`slint-tr-extractor`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/translations.mdx)
  — generate `messages.pot` from `.slint` files; diff against the
  checked-in `.pot` in CI.

## 1. `slint-viewer` workflow

Document a one-command per-page preview:

```bash
# Install once
cargo install slint-viewer --version "=1.15.1"

# Preview the whole app (mounts MainWindow)
slint-viewer ui/main.slint

# Preview a single page in isolation
slint-viewer ui/pages/media_backend_page.slint --component MediaBackendPage

# Live-reload on every save
slint-viewer ui/main.slint --auto-reload
```

`slint-viewer` lets you toggle backend state via the property-inspector
panel. For pages that depend on `MediaBackend.state`, manually set the
enum from the viewer's right pane.

> The `slint-viewer` version **must** match the Slint pin in `Cargo.toml`
> — viewer-vs-runtime mismatches produce confusing errors. Add a check:
>
> ```bash
> # scripts/check-slint-viewer.sh
> set -euo pipefail
> pinned=$(awk -F\" '/^slint = / { print $2 }' Cargo.toml | head -1)
> have=$(slint-viewer --version | awk '{ print $2 }')
> if [[ "$pinned" != "$have" ]]; then
>     echo "Pinned Slint $pinned but slint-viewer is $have. Run: cargo install slint-viewer --version =$pinned --force"
>     exit 1
> fi
> ```

## 2. CI lint — catch the regressions this guide fixes

Add `.github/workflows/ui-lint.yml`:

```yaml
name: ui-lint
on:
  pull_request:
    paths: ['ui/**', 'docs/ui-slint-best-practices/**']

jobs:
  ui-lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Forbid raw hex colours in FCast Slint files
        run: |
          set -euo pipefail
          ! git grep -nE '#[0-9a-fA-F]{3,8}' \
              -- 'ui/components/*.slint' 'ui/pages/*.slint' \
                 'ui/main.slint' 'ui/bridge.slint' \
                 ':!ui/components/qr_placeholder.slint'

      - name: Forbid hard-coded font-size
        run: |
          ! git grep -nE 'font-size: [0-9]+px' \
              -- 'ui/components/*.slint' 'ui/pages/*.slint'

      - name: Forbid value-compares against @tr in ComboBoxes
        run: ! git grep -nF 'value == @tr(' ui/

      - name: Forbid direct Bridge.active-panel writes
        run: |
          # PanelBridge.active is fine; only PanelBridge.{push,pop,replace} should write.
          ! git grep -nE 'Bridge\.active-panel\s*=' ui/

      - name: Forbid setting Bridge.lifecycle directly
        run: |
          # Use AppBridge.exit-lifecycle() instead.
          ! git grep -nE 'Bridge\.lifecycle\s*=' ui/

      - name: Translations stay in sync
        run: |
          cargo install slint-tr-extractor --version "=1.15.1" --quiet
          slint-tr-extractor ui/main.slint -o /tmp/messages.pot
          # Compare modulo header lines (date / version metadata).
          diff <(sed -E '/^"(POT-Creation-Date|Project-Id-Version)/d' /tmp/messages.pot) \
               <(sed -E '/^"(POT-Creation-Date|Project-Id-Version)/d' ui/i18n/messages.pot)
```

> The `!` prefix in the shell step turns a non-zero exit (grep found
> something) into a failed CI step.

Each "forbid" rule corresponds to one of the steps in this guide. If
you intentionally skip a step, drop the matching rule.

## 3. Snapshot tests via `i-slint-backend-testing`

Add a `senders/android/tests/ui_snapshots.rs` (or new
`crates/ui-tests/` if you want it separate from the Android crate):

```rust
//! Render-only checks: instantiate the app under the headless backend
//! and dump the accessibility tree + a screenshot. Compare against
//! the golden tree in `tests/snapshots/`.

use slint::ComponentHandle;

slint::include_modules!();

fn with_headless<F: FnOnce()>(f: F) {
    // i-slint-backend-testing routes Window::show into an off-screen
    // surface and provides a 1px-accurate snapshot.
    i_slint_backend_testing::init_no_event_loop();
    f();
}

#[test]
fn connect_view_accessible_tree() {
    with_headless(|| {
        let ui = MainWindow::new().unwrap();
        ui.global::<AppBridge>().set_app_state(AppState::Disconnected);
        ui.global::<Receivers>().set_devices(slint::ModelRc::new(slint::VecModel::from(vec![])));
        ui.show().unwrap();

        let dump = i_slint_backend_testing::accessibility_tree(&ui);
        let golden = include_str!("snapshots/connect_view_empty.a11y.txt");
        assert_eq!(dump.trim(), golden.trim(),
                   "ConnectView a11y tree drifted. Update snapshot if intentional.");
    });
}

#[test]
fn media_backend_states_a11y() {
    with_headless(|| {
        let ui = MainWindow::new().unwrap();
        ui.global::<PanelBridge>().set_active(Panel::SettingsMediaBackend);

        for state in [MediaBackendState::Disconnected,
                       MediaBackendState::Probing,
                       MediaBackendState::Ready,
                       MediaBackendState::Error] {
            ui.global::<MediaBackend>().set_state(state);
            let label = i_slint_backend_testing::accessible_label_at(&ui, "StatusPill");
            assert!(label.contains(match state {
                MediaBackendState::Disconnected => "Disconnected",
                MediaBackendState::Probing      => "Probing",
                MediaBackendState::Ready        => "Ready",
                MediaBackendState::Error        => "Error",
            }), "Unexpected status pill label for state {:?}: {}", state, label);
        }
    });
}

#[test]
fn back_key_pops_panel_stack() {
    with_headless(|| {
        let ui = MainWindow::new().unwrap();
        let panels = ui.global::<PanelBridge>();

        panels.invoke_push(Panel::SettingsRoot);
        panels.invoke_push(Panel::SettingsMediaBackend);

        // Simulate the platform back-key (Escape under the testing backend).
        i_slint_backend_testing::send_key_press(&ui, slint::SharedString::from("\u{1b}"));

        assert_eq!(panels.get_active(), Panel::SettingsRoot);
    });
}
```

Add `i-slint-backend-testing` as a `[dev-dependencies]` entry in the
relevant `Cargo.toml`:

```toml
[dev-dependencies]
i-slint-backend-testing = { version = "1.15.1" }
```

Pin to the Slint version. The backend is headless and works in CI
without an X server / display.

## 4. Accessibility-tree golden files

The snapshot golden for `connect_view_empty.a11y.txt` reads roughly:

```text
- accessible-role: text                 label: "Devices"
- accessible-role: text                 label: "No receivers found"
- accessible-role: button               label: "Refresh"
- accessible-role: button               label: "Scan QR"
```

Generated by:

```bash
cargo test -p android-sender ui_snapshots -- --nocapture --ignored \
    --test-threads=1 dump_accessibility
```

Commit the generated text files into
`senders/android/tests/snapshots/`. Subsequent runs assert no drift.

When a snapshot legitimately needs to change, regenerate and review the
diff in the PR.

## 5. Visual snapshot via screenshot

If you want pixel-exact regression detection (not just a11y), the
testing backend can write a PNG:

```rust
fn render_to_png(ui: &MainWindow, path: &str) {
    let img = i_slint_backend_testing::render_to_image(ui, 1080, 1920);
    img.save(path).unwrap();
}

#[test]
fn lock_overlay_visual() {
    with_headless(|| {
        let ui = MainWindow::new().unwrap();
        ui.global::<AppBridge>().set_lifecycle(LifecycleMode::Locked);
        render_to_png(&ui, "tests/snapshots/lock_overlay.png");
    });
}
```

In CI, compare the generated PNG against the committed golden using
`oxipng --compare` or a hand-rolled byte-compare. Pixel comparisons
across GPU drivers can flake — limit visual snapshots to overlays
without text rendering (lock overlay, snapshot countdown, info banner)
to minimise flake.

## 6. Pre-commit hook for the lint rules

Wire the same checks into pre-commit so authors don't push obvious
violations:

```yaml
# .pre-commit-config.yaml (excerpt)
repos:
  - repo: local
    hooks:
      - id: forbid-raw-hex-colors
        name: Forbid raw hex colors in FCast Slint files
        entry: bash -c '! git grep -nE "#[0-9a-fA-F]{3,8}" -- "ui/components/*.slint" "ui/pages/*.slint" "ui/main.slint" "ui/bridge.slint" ":!ui/components/qr_placeholder.slint"'
        language: system
        pass_filenames: false

      - id: forbid-direct-active-panel-writes
        name: Forbid direct Bridge.active-panel writes
        entry: bash -c '! git grep -nE "Bridge\\.active-panel\\s*=" ui/'
        language: system
        pass_filenames: false
```

The hook installs with `pre-commit install` (already in the repo —
the gstpop guide step on pre-commit covers the setup).

## 7. Documentation: how to update golden files

Drop a `senders/android/tests/snapshots/README.md`:

```markdown
# UI snapshot goldens

Golden files for `i-slint-backend-testing` snapshot tests.

## Refresh
```bash
UI_SNAPSHOT_REFRESH=1 cargo test -p android-sender ui_snapshots
```

The tests update the golden file on disk when the env var is set; review
the diff and commit.

## Categories
- `*.a11y.txt`  — accessibility tree dumps (text)
- `*.png`       — pixel-exact screenshots (only overlays / non-text panels)
```

## Migration

1. Document `slint-viewer` setup in the project README (under a new
   "UI development" heading).
2. Add `.github/workflows/ui-lint.yml` with the grep-based rules.
3. Add `senders/android/tests/ui_snapshots.rs` + initial golden files
   for: `ConnectView` empty, `MediaBackendPage` × 4 states, lock
   overlay.
4. Add the pre-commit hooks.
5. Run the snapshot suite locally; iterate until green.
6. Land the workflow in the same PR as the first big refactor (step
   01 is a good fit — colour tokens map directly to the
   forbid-raw-hex rule).

### Per-file checklist

| File                                                    | Action                              |
| ------------------------------------------------------- | ----------------------------------- |
| `.github/workflows/ui-lint.yml`                         | NEW                                 |
| `.pre-commit-config.yaml`                               | Add forbid-raw-* hooks              |
| `senders/android/tests/ui_snapshots.rs`                 | NEW                                 |
| `senders/android/tests/snapshots/*.a11y.txt`            | NEW (commit goldens)                |
| `senders/android/Cargo.toml`                            | Add `i-slint-backend-testing` dev-dep |
| `scripts/check-slint-viewer.sh`                         | NEW (used by `make ui-dev`)         |
| `README.md` / `CONTRIBUTING.md`                         | Add "UI development" section        |

## Out of scope

- Full visual-regression infrastructure (Percy, Chromatic).
  Overkill for this codebase.
- Cross-platform pixel snapshots. Pin to one CI runner OS for the PNG
  goldens.
- AI-driven UI exploration. The headless backend has APIs for
  programmatic click sequences — add focused tests rather than fuzz.

## Acceptance

- [ ] `cargo test -p android-sender ui_snapshots` passes locally and
      in CI.
- [ ] `slint-viewer ui/main.slint --component MediaBackendPage`
      renders all four states from the property inspector.
- [ ] CI rejects a PR that re-introduces `Bridge.active-panel =
      Panel.none` in any file. Verify by submitting a deliberate
      regression in a draft PR.
- [ ] A non-trivial Slint change (e.g. flipping a colour) regenerates a
      visible diff in the golden file under
      `UI_SNAPSHOT_REFRESH=1 cargo test`.
