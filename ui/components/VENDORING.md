# Vendored Slint helpers

These files are copied from `kodyka/fcast` because Slint imports must resolve
from the local filesystem at compile time.

Pinned source commit:

- `63980e6736e65adbd15588d21903d0c02223c15c`

Source mapping:

- `mcore/common.slint` ← `sdk/mirroring_core/ui/common.slint`
- `std/` ← `senders/ui-components/`

After copying fresh sources from the monorepo, re-apply these local rewrites:

- `ui/pages/settings_page.slint`
  `../../../../sdk/mirroring_core/ui/common.slint` → `../components/mcore/common.slint`
- `ui/components/mcore/common.slint`
  `../../../senders/ui-components/std-widgets.slint` → `../std/std-widgets.slint`
