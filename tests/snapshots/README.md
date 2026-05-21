# UI snapshot goldens

Golden files for `i-slint-backend-testing` snapshot tests (`tests/ui_snapshots.rs`).

## Run the tests

```bash
cargo test --test ui_snapshots
```

## Refresh golden files

Set `UI_SNAPSHOT_REFRESH=1` and re-run:

```bash
UI_SNAPSHOT_REFRESH=1 cargo test --test ui_snapshots
```

Review the diff and commit the updated goldens.

## Preview pages with slint-viewer

Use `nix-shell` to run slint-viewer without a global install:

```bash
# Preview the full app
nix-shell -p slint-viewer --run "slint-viewer ui/main.slint --auto-reload"

# Preview a single page in isolation
nix-shell -p slint-viewer --run "slint-viewer ui/pages/media_backend_page.slint --component MediaBackendPage"
```

The viewer version in nixpkgs must match the Slint pin in `Cargo.toml`.
Run `scripts/check-slint-viewer.sh` to verify.

## File categories

| Extension   | Description                                    |
| ----------- | ---------------------------------------------- |
| `*.a11y.txt`| Accessibility tree dumps (text)                |
| `*.png`     | Pixel-exact screenshots (overlays only)        |
