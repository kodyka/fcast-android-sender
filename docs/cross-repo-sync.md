# Cross-repo workflow: `fcast-android-sender` and `kodyka/fcast`

The Android sender depends on three Rust crates that remain in the FCast
monorepo (`kodyka/fcast`):

- `fcast-protocol` (`sdk/common/fcast-protocol`)
- `fcast-sender-sdk` (`sdk/sender/fcast-sender-sdk`)
- `mcore` (`sdk/mirroring_core`)

They are pinned as Git dependencies to a single monorepo commit SHA.

## When to bump the SDK pin

- Routine: weekly or whenever accumulated drift becomes annoying
- On demand: any Android sender change that needs a newer SDK fix or API
- Coordinated: when a feature spans both repos

Keep all three Git dependencies on the same `rev`.

## How to bump the SDK pin

1. Choose the new monorepo SHA.
2. Update all three `rev = "..."` entries in `Cargo.toml`.
3. Run `cargo update -p fcast-protocol -p fcast-sender-sdk -p mcore`.
4. Re-run `cargo check --target aarch64-linux-android`.
5. If `sdk/mirroring_core/ui/` or `senders/ui-components/` changed upstream,
   re-vendor the Slint helpers described below.

Suggested PR title:

`chore(sdk): bump fcast SDK pin to <sha-prefix>`

## Cross-repo PR pair workflow

If a feature needs both SDK work and Android sender work:

1. Land the SDK change in `kodyka/fcast`.
2. Record the merged commit SHA.
3. Bump the Android sender repo to that SHA.
4. Land the consumer-side changes in the same PR as the bump.

Do not point the Android sender at an unmerged SDK commit. If the SDK branch is
rebased away, the Git dependency becomes unreachable.

## Re-vendoring the Slint helpers

The standalone repo vendors:

- `ui/components/mcore/common.slint` from `sdk/mirroring_core/ui/common.slint`
- `ui/components/std/` from `senders/ui-components/`

Re-vendor with:

```bash
SRC=/path/to/fcast
cp "$SRC/sdk/mirroring_core/ui/common.slint" ui/components/mcore/common.slint
cp -a "$SRC/senders/ui-components/." ui/components/std/
```

Then re-apply the two local import rewrites:

- `ui/pages/settings_page.slint` imports `../components/mcore/common.slint`
- `ui/components/mcore/common.slint` imports `../std/std-widgets.slint`

## Release-blocking regression

If a pin bump breaks the Android sender late in a release cycle:

1. Revert the bump PR.
2. Fix the SDK in `kodyka/fcast`.
3. Retry the bump with the fixed SHA.

Avoid carrying a private fork of the SDK inside this repo. The drift cost is
higher than the revert-and-retry cost.
