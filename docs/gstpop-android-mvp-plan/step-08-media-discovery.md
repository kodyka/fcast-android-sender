# Step 8 — Media discovery wrapper

**Phase:** 2 — Android polish
**Priority:** medium
**Depends on:** Step 4
**Unblocks:** richer UI metadata

## Goal

Expose `gstpop::gst::discoverer::discover_uri()` through `gstpop-runtime`
behind the existing `media-tools` feature so the app can fetch duration,
streams, caps, and basic playability info before starting playback.

## Files touched

- `crates/gstpop-runtime/src/media.rs` (extend)
- `crates/gstpop-runtime/src/lib.rs` (re-exports)

## Implementation

### 1. Re-export helpers from `media.rs`

Append to `crates/gstpop-runtime/src/media.rs`:

```rust
use std::path::Path;
use std::time::Duration;

/// Result of a `discover()` call. Mirrors the subset of the vendored
/// `DiscovererInfo` fields most relevant to UI code.
#[derive(Debug, Clone)]
pub struct DiscoverResult {
    pub uri: String,
    pub duration: Option<Duration>,
    pub seekable: bool,
    pub video_streams: usize,
    pub audio_streams: usize,
    pub subtitle_streams: usize,
    pub tags: Vec<(String, String)>,
}

/// Discover media metadata for `input`, normalising the path first.
///
/// `base_dir` follows the same rule as
/// [`normalise_media_input`]: `Some(...)` is required on Android.
/// `timeout` defaults to 5 seconds when `None`.
pub async fn discover(
    input: &str,
    base_dir: Option<&Path>,
    timeout: Option<Duration>,
) -> anyhow::Result<DiscoverResult> {
    let uri = normalise_media_input(input, base_dir)?;
    let timeout = timeout.unwrap_or(Duration::from_secs(5));

    // Run the synchronous gstreamer discoverer on a blocking thread so we
    // don't stall the tokio runtime.
    let uri_for_blocking = uri.clone();
    let info = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        gstpop::gst::discoverer::discover_uri(&uri_for_blocking, timeout)
            .map_err(|e| anyhow::anyhow!("discover failed: {e:#}"))
    })
    .await
    .map_err(|e| anyhow::anyhow!("discoverer join failed: {e}"))??;

    Ok(DiscoverResult {
        uri,
        duration: info.duration,
        seekable: info.seekable,
        video_streams: info.video_streams,
        audio_streams: info.audio_streams,
        subtitle_streams: info.subtitle_streams,
        tags: info.tags,
    })
}
```

> **Adjust field names** to match what `vendor/gstpop/src/gst/discoverer.rs`
> actually exposes. Verify with `grep -n 'pub' vendor/gstpop/src/gst/discoverer.rs`
> before pasting; the names above follow the upstream `DiscovererInfo` shape.

### 2. Re-export

```rust
// crates/gstpop-runtime/src/lib.rs
#[cfg(feature = "media-tools")]
pub use media::{
    build_playbin_description, discover, normalise_media_input,
    DiscoverResult,
};
```

## Tests

Append to `media.rs`'s `#[cfg(test)] mod tests` (gated to skip on hosts
without `gstreamer-libav`):

```rust
#[tokio::test]
#[ignore = "requires gstreamer plugins on host; run locally with --ignored"]
async fn discover_resolves_local_audio_file() {
    // generate a tiny WAV with sox or ffmpeg before running the test, or
    // skip if the fixture is missing.
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/silence.wav");
    if !fixture.exists() {
        eprintln!("skip: fixture {} missing", fixture.display());
        return;
    }
    let info = discover(
        fixture.to_str().unwrap(),
        None,
        Some(std::time::Duration::from_secs(2)),
    )
    .await
    .expect("discover");
    assert!(info.uri.starts_with("file://"));
    assert!(info.audio_streams >= 1);
}
```

## Android usage sketch

The JNI shim from [Step 7](./step-07-jni-bridge.md) does **not** expose
`discover`. Either:

- Add a fourth JNI method `nativeDiscover(input: String, baseDir: String): String`
  returning JSON; or
- Add a server-side RPC (e.g. `discover`) in `vendor/gstpop`'s WebSocket
  manager and call it from Kotlin via the typed client.

The second option keeps JNI minimal and is preferred.

## Verification

```bash
cargo build -p gstpop-runtime --features media-tools
cargo test  -p gstpop-runtime --features media-tools --lib media -- --ignored
```

## Done when

- `gstpop_runtime::discover(...)` is callable behind `media-tools`.
- A local-file discovery returns non-empty stream counts on the host.
- Android arm64 build still succeeds with the feature enabled.
