# Step 10 — Desktop tooling feature

**Phase:** 3 — Desktop & cross-platform
**Priority:** medium/low
**Depends on:** Step 3
**Unblocks:** developer ergonomics, debugging

## Goal

Expose registry inspection, element introspection, and the upstream
`inspect_format` text formatter from `vendor/gstpop` through `gstpop-runtime`
behind a `desktop-tools` feature. Default Android builds stay free of these
symbols.

## Files touched

- `crates/gstpop-runtime/Cargo.toml` (add `desktop-tools` feature)
- `crates/gstpop-runtime/src/inspect.rs` (new)
- `crates/gstpop-runtime/src/lib.rs` (cfg-gated module)

## Implementation

### 1. Feature flag

```toml
[features]
default = []
typed-client = []
media-tools = []
desktop-tools = []
android-jni = ["dep:jni", "dep:ndk-context"]
```

### 2. Wrapper module

Create `crates/gstpop-runtime/src/inspect.rs`:

```rust
//! Thin façade over `gstpop::gst::registry` and `gstpop::gst::inspect_format`.
//! Compiled only with the `desktop-tools` feature; do not enable in Android
//! release builds.

use anyhow::Result;

/// Lightweight element descriptor for UI listings.
#[derive(Debug, Clone)]
pub struct ElementSummary {
    pub name: String,
    pub long_name: String,
    pub klass: String,
    pub description: String,
    pub author: String,
    pub rank: i32,
}

/// List all registered GStreamer elements.
pub fn list_elements() -> Result<Vec<ElementSummary>> {
    let raw = gstpop::gst::registry::get_elements()
        .map_err(|e| anyhow::anyhow!("registry: {e:#}"))?;
    Ok(raw
        .into_iter()
        .map(|e| ElementSummary {
            name: e.name,
            long_name: e.long_name,
            klass: e.klass,
            description: e.description,
            author: e.author,
            rank: e.rank,
        })
        .collect())
}

/// Inspect a single element by name. `None` if not in the registry.
pub fn inspect_element(name: &str) -> Result<Option<String>> {
    match gstpop::gst::registry::get_element(name) {
        Ok(Some(info)) => Ok(Some(
            gstpop::gst::inspect_format::format_element(&info),
        )),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("inspect {name}: {e:#}")),
    }
}
```

> Adjust field names to whatever `vendor/gstpop/src/gst/registry.rs` actually
> exposes — verify with `grep -n pub vendor/gstpop/src/gst/registry.rs`
> before pasting.

### 3. Wire into `lib.rs`

```rust
#[cfg(feature = "desktop-tools")]
pub mod inspect;

#[cfg(feature = "desktop-tools")]
pub use inspect::{inspect_element, list_elements, ElementSummary};
```

## Example desktop tool

Create `crates/gstpop-runtime/examples/inspect_cli.rs`:

```rust
//! Tiny desktop helper: `cargo run --example inspect_cli --features desktop-tools -- videotestsrc`

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1);
    gstreamer::init()?;

    match arg.as_deref() {
        None => {
            for e in gstpop_runtime::list_elements()? {
                println!("{:>6} {:<30} {}", e.rank, e.name, e.long_name);
            }
        }
        Some(name) => match gstpop_runtime::inspect_element(name)? {
            Some(text) => println!("{text}"),
            None => {
                eprintln!("no such element: {name}");
                std::process::exit(1);
            }
        },
    }
    Ok(())
}
```

Add to `crates/gstpop-runtime/Cargo.toml`:

```toml
[[example]]
name = "inspect_cli"
required-features = ["desktop-tools"]
```

## Tests

```rust
// crates/gstpop-runtime/src/inspect.rs (append)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_at_least_one_element() {
        gstreamer::init().unwrap();
        let elements = list_elements().unwrap();
        assert!(!elements.is_empty(), "registry should be non-empty");
    }

    #[test]
    fn inspect_unknown_returns_none() {
        gstreamer::init().unwrap();
        assert!(inspect_element("definitely_not_an_element_xyz").unwrap().is_none());
    }
}
```

## Verification

```bash
cargo build -p gstpop-runtime --features desktop-tools
cargo test  -p gstpop-runtime --features desktop-tools --lib inspect
cargo run   -p gstpop-runtime --features desktop-tools --example inspect_cli -- videotestsrc
```

## Important: keep out of Android builds

- Do **not** add `desktop-tools` to any default Android build command.
- Verify the arm64 release build does NOT pull in this module:

  ```bash
  cargo ndk -t arm64-v8a build -p gstpop-runtime \
      --features "typed-client media-tools" --release
  ```

  Should compile cleanly without `desktop-tools`.

## Done when

- `desktop-tools` feature exposes `list_elements`, `inspect_element`,
  `ElementSummary`.
- `inspect_cli` example runs and prints registry contents on the host.
- Android arm64 build remains unaffected.
