# Step 4 — Android-safe media path handling

**Phase:** 1 — Android MVP
**Priority:** highest
**Depends on:** nothing
**Unblocks:** Step 8 (media discovery)

## Goal

Provide a media-input normaliser the app can call **without** depending on
`std::env::current_dir()`. The Android caller must pass an explicit base
directory (e.g. `Context.filesDir`); desktop callers may pass `None` to
preserve upstream behavior.

Also expose a `build_playbin_description` analogue with the same explicit
base-dir signature and sink-name validation.

## Files touched

- `crates/gstpop-runtime/Cargo.toml` (add `media-tools` feature)
- `crates/gstpop-runtime/src/media.rs` (new)
- `crates/gstpop-runtime/src/lib.rs` (feature-gated module)

## Implementation

### 1. Feature flag

```toml
# crates/gstpop-runtime/Cargo.toml
[features]
default = []
typed-client = []
media-tools = []
```

### 2. The module

Create `crates/gstpop-runtime/src/media.rs`:

```rust
//! Android-safe media path & playbin description helpers.
//!
//! Upstream `gstpop::gst::discoverer::normalize_uri` resolves bare paths
//! against `std::env::current_dir()`. That is correct for desktop CLI use
//! but wrong for Android, where the working directory is typically `/` and
//! app-owned files live under `Context.filesDir` /
//! `Context.getExternalFilesDir(null)`. This module requires the caller to
//! supply an explicit base directory on Android.

use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// Normalise a user-provided media input into a `file://` or pass-through URI.
///
/// - If `input` already contains `://`, returned as-is.
/// - If `input` is absolute, used directly.
/// - If `input` is relative and `base_dir` is `Some`, joined onto it.
/// - If `input` is relative and `base_dir` is `None`, falls back to
///   `std::env::current_dir()` (desktop only; never pass `None` from Android).
pub fn normalise_media_input(input: &str, base_dir: Option<&Path>) -> Result<String> {
    let input = input.trim();
    if input.is_empty() {
        bail!("empty media input");
    }
    if input.contains("://") {
        return Ok(input.to_owned());
    }

    let path = Path::new(input);
    let absolute: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = base_dir {
        base.join(path)
    } else {
        #[cfg(target_os = "android")]
        {
            bail!(
                "relative media path {input:?} requires an explicit base_dir on Android"
            );
        }
        #[cfg(not(target_os = "android"))]
        {
            std::env::current_dir()?.join(path)
        }
    };

    // canonicalize is best-effort: paths that don't exist yet (e.g. about to
    // be written) should still produce a valid file:// URI.
    let absolute = absolute.canonicalize().unwrap_or(absolute);
    let path_str = absolute.to_string_lossy().replace('\\', "/");

    if path_str.starts_with('/') {
        Ok(format!("file://{path_str}"))
    } else {
        // Windows drive-letter form, e.g. C:/Users/...
        Ok(format!("file:///{path_str}"))
    }
}

/// Build a `playbin` / `playbin3` description string for `gst-launch`-style
/// pipeline creation. Validates sink element names to avoid shell-style
/// injection into the pipeline description grammar.
pub fn build_playbin_description(
    input: &str,
    base_dir: Option<&Path>,
    video_sink: Option<&str>,
    audio_sink: Option<&str>,
    use_legacy_playbin: bool,
) -> Result<String> {
    let uri = normalise_media_input(input, base_dir)?;
    if let Some(vs) = video_sink {
        validate_sink_name(vs)?;
    }
    if let Some(asink) = audio_sink {
        validate_sink_name(asink)?;
    }

    let element = if use_legacy_playbin { "playbin" } else { "playbin3" };
    let escaped_uri = uri.replace('"', r#"\""#);

    let mut desc = format!(r#"{element} uri="{escaped_uri}""#);
    if let Some(vs) = video_sink {
        desc.push_str(&format!(" video-sink={vs}"));
    }
    if let Some(asink) = audio_sink {
        desc.push_str(&format!(" audio-sink={asink}"));
    }
    Ok(desc)
}

fn validate_sink_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("empty sink element name");
    }
    const FORBIDDEN: &[char] = &['!', '"', '\'', '`', '\\', ' ', '\t', '\n'];
    if name.contains(FORBIDDEN) {
        bail!("invalid sink element name: {name}");
    }
    Ok(())
}
```

### 3. Wire into `lib.rs`

```rust
#[cfg(feature = "media-tools")]
pub mod media;
```

## Tests

Append to `media.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn passthrough_uri() {
        assert_eq!(
            normalise_media_input("https://example.com/a.mp4", None).unwrap(),
            "https://example.com/a.mp4",
        );
    }

    #[test]
    fn absolute_unix_path() {
        let out = normalise_media_input("/tmp/x.mp4", None).unwrap();
        assert!(out.starts_with("file:///") || out.starts_with("file://"), "got {out}");
        assert!(out.ends_with("/x.mp4"), "got {out}");
    }

    #[test]
    fn relative_with_base() {
        let base = PathBuf::from("/data/user/0/org.fcast.sender/files");
        let out = normalise_media_input("clip.mp4", Some(&base)).unwrap();
        assert_eq!(out, "file:///data/user/0/org.fcast.sender/files/clip.mp4");
    }

    #[test]
    #[cfg(target_os = "android")]
    fn relative_without_base_errors_on_android() {
        assert!(normalise_media_input("clip.mp4", None).is_err());
    }

    #[test]
    fn empty_input_errors() {
        assert!(normalise_media_input("   ", None).is_err());
    }

    #[test]
    fn rejects_bad_sink_names() {
        assert!(build_playbin_description(
            "/tmp/a.mp4", None, Some("bad sink"), None, false
        ).is_err());
        assert!(build_playbin_description(
            "/tmp/a.mp4", None, Some("with\"quote"), None, false
        ).is_err());
    }

    #[test]
    fn builds_playbin3_description() {
        let desc = build_playbin_description(
            "/tmp/a.mp4",
            None,
            Some("autovideosink"),
            Some("autoaudiosink"),
            false,
        )
        .unwrap();
        assert!(desc.starts_with("playbin3 uri=\"file://"), "got {desc}");
        assert!(desc.contains("video-sink=autovideosink"));
        assert!(desc.contains("audio-sink=autoaudiosink"));
    }
}
```

## Android usage sketch

Kotlin side (informational — actual JNI in [Step 7](./step-07-jni-bridge.md)):

```kotlin
val baseDir = context.filesDir.absolutePath   // e.g. /data/user/0/.../files
val uri = GstPopBridge.normaliseMediaInput("clip.mp4", baseDir)
val desc = GstPopBridge.buildPlaybinDescription(uri, null, null, false)
```

## Verification

```bash
cargo build -p gstpop-runtime --features media-tools
cargo test  -p gstpop-runtime --features media-tools --lib media
```

## Done when

- `normalise_media_input` errors on relative paths without `base_dir` when
  compiled for `target_os = "android"`.
- Sink-name validation rejects whitespace, quotes, backslashes, and bangs.
- All unit tests pass on host and (after Step 6) on device.
