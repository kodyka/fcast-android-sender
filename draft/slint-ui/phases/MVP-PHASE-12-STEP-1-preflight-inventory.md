# MVP-PHASE-12 — Step 1: Pre-flight inventory

> Part 1 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Next: [STEP-2](./MVP-PHASE-12-STEP-2-bridge-data-model.md).

---

## 0. Goal of this step

Verify that every line-number anchor cited in the parent doc and the
rest of the STEP files still matches the current tree. Re-run this
ladder whenever you pick the work back up after a few days away — line
drift is the single most common doc bug.

This step adds **zero source lines**. It is a checklist.

---

## 1. Anchor ladder

Run each grep below from the repo root. Each row tells you what the
output **must** look like; if it doesn't, stop and update the anchors
in the relevant step file before proceeding.

| # | Anchor | Command | Expected (substring) |
|---|---|---|---|
| 1.1 | `Command` enum (lowercase variants) | `grep -nE '^#\[serde\(rename_all = "lowercase"\)\]' src/migration/protocol.rs` | `49:#[serde(rename_all = "lowercase")]` |
| 1.2 | `CreateSource` / `CreateMixer` / `Connect` / `Start` variants | `grep -nE 'CreateSource \{\|CreateMixer \{\|^    Connect \{\|^    Start \{' src/migration/protocol.rs` | Four hits in the 50-110 range |
| 1.3 | `DestinationFamily::Rtmp` (referenced by both backends) | `grep -n 'Rtmp \{' src/migration/protocol.rs` | `~149:    Rtmp {` |
| 1.4 | JNI bridge for migration commands | `grep -n 'fn run_graph_command' src/lib.rs` | `218:fn run_graph_command(action: &str, params: Value)` |
| 1.5 | Migration runtime entrypoints | `grep -n 'pub fn start_graph_runtime\|pub fn shutdown_graph_runtime' src/migration/runtime.rs` | `302:` and `312:` |
| 1.6 | Migration runtime JSON dispatcher | `grep -n 'pub fn try_handle_command_json' src/migration/runtime.rs` | `349:` |
| 1.7 | `Bridge` global root | `grep -n 'export global Bridge' ui/bridge.slint` | `142:export global Bridge` (or near — adjust STEP-2 if the line moved) |
| 1.8 | `Panel` enum + last variant | `grep -n '^export enum Panel\|^    network,$\|^}' ui/bridge.slint \| head -3` | Confirms `network` is the current last variant; STEP-4 appends `media-backend` |
| 1.9 | `FullSettingsPage` "AUDIO & VIDEO" section (sibling of where we'll insert) | `grep -n 'SettingsSection \{' ui/pages/settings_page.slint` | Multiple hits around lines 109–254; STEP-3 inserts a new section below "CODEC & DEBUG" |
| 1.10 | `SettingsSection`, `SettingsToggleRow`, `SettingsValueRow` exports | `grep -nE '^export component (SettingsSection\|SettingsToggleRow\|SettingsValueRow)' ui/components/settings_rows.slint` | Three exports |
| 1.11 | `Theme` tokens used by the new page | `grep -nE 'spacing-default\|padding-screen\|radius-card\|surface-card\|font-size-(heading\|body\|label)\|text-(primary\|secondary)\|error-fg\|accent' ui/theme.slint \| head -10` | At least 10 hits — every token cited in STEP-4 must exist |
| 1.12 | `tokio` already on `"full"` features | `grep -n '^tokio' Cargo.toml` | `19:tokio = { version = "1.51", features = ["full"] }` — STEP-9 only adds `tokio-tungstenite`, not `tokio` itself |
| 1.13 | Cast-history JNI hook (parallel for backend hooks) | `grep -n 'on_recast\\|on_run_quick_action' src/lib.rs \| head -5` | Confirms PHASE-9 callback-registration convention; STEP-8 mirrors it |
| 1.14 | `slint::ComponentHandle` pattern + `weak.upgrade_in_event_loop` | `grep -n 'upgrade_in_event_loop' src/lib.rs \| head -5` | Multiple hits — every async backend handler in STEP-8 uses this pattern |

---

## 2. Verify the gst-pop docs are still current

```sh
# Read-only — no checkout into the repo. The gst-pop daemon README is
# the source of truth for the JSON-RPC method names and event payload
# shapes used in STEP-6 and STEP-7. Pin the SHA in your notes if you
# want byte-stable references.
curl -sL https://raw.githubusercontent.com/dabrain34/gstpop/main/daemon/README.md \
  | grep -nE '^####? `(create_pipeline|set_state|update_pipeline|remove_pipeline|get_pipeline_info|get_position|play|pause|stop|snapshot|get_version|get_info|get_pipeline_count|list_pipelines)`' \
  | head -20
```

Expected: 13 method headings appear in the order listed in the parent
doc's §0 table.

Also verify the event names STEP-8 subscribes to:

```sh
curl -sL https://raw.githubusercontent.com/dabrain34/gstpop/main/daemon/README.md \
  | grep -nE '^#### `(state_changed|error|unsupported|eos|pipeline_added|pipeline_updated|pipeline_removed)`'
```

Expected: 7 event headings.

---

## 3. Verify there's no existing `src/backend/` module

```sh
ls src/backend 2>&1 | head -3   # → "ls: cannot access 'src/backend': No such file or directory"
grep -rn '^mod backend;' src/lib.rs | head -3   # → no output
```

If `src/backend/` already exists from an earlier exploration branch,
delete it (or rename) before starting STEP-5; the guide assumes a clean
slate.

---

## 4. Optional: verify no name collisions with existing settings rows

`MediaBackend` is the name STEP-2 introduces for the new enum. Make
sure no other Slint type/global shadows it:

```sh
grep -rn '\bMediaBackend\b' ui/ src/ 2>&1 | head -5   # → expected no output
```

If a hit appears, rename the new enum in STEP-2 (e.g. to
`StreamBackend`) and update every downstream STEP file.

---

## 5. Exit gate

- [ ] All 14 grep rows in §1 produce the expected hits.
- [ ] §2 confirms the gst-pop docs still list every method/event we
      target.
- [ ] §3 confirms `src/backend/` does not exist yet.
- [ ] §4 confirms `MediaBackend` is a free name.

When all four boxes are checked, proceed to
[STEP-2](./MVP-PHASE-12-STEP-2-bridge-data-model.md).
