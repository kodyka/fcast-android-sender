# Step 12 — Separate desktop CLI crate

**Phase:** 3 — Desktop & cross-platform
**Priority:** low
**Depends on:** Steps 1, 3, 10
**Unblocks:** desktop parity with upstream `gst-pop` daemon

## Goal

Port the upstream CLI subcommands (`daemon`, `play`, `launch`, `discover`,
`inspect`) into a **new** `crates/gstpop-cli` crate. Keep `clap`,
`tracing-subscriber`, and signal handling out of `gstpop-runtime` so the
mobile build stays lean.

## Files touched

- `crates/gstpop-cli/Cargo.toml` (new)
- `crates/gstpop-cli/src/main.rs` (new)
- `crates/gstpop-cli/src/cmd/{daemon,play,launch,discover,inspect}.rs` (new)
- root `Cargo.toml` (workspace members)

## Crate skeleton

`crates/gstpop-cli/Cargo.toml`:

```toml
[package]
name = "gstpop-cli"
version = "0.1.0"
edition = "2021"
publish = false

[[bin]]
name = "gstpop"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
clap = { version = "4", features = ["derive"] }
gstpop = { path = "../../vendor/gstpop" }
gstpop-runtime = { path = "../gstpop-runtime", features = [
    "typed-client",
    "media-tools",
    "desktop-tools",
] }
serde.workspace = true
serde_json.workspace = true
tokio = { workspace = true, features = ["full"] }
tracing.workspace = true
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

Add to root `Cargo.toml`:

```toml
[workspace]
members = [
    # ...existing entries...
    "crates/gstpop-cli",
]
```

## Main dispatch

`crates/gstpop-cli/src/main.rs`:

```rust
use clap::{Parser, Subcommand};

mod cmd;
mod signal;

#[derive(Parser)]
#[command(name = "gstpop", about = "Desktop CLI for gstpop")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Daemon(cmd::daemon::Args),
    Play(cmd::play::Args),
    Launch(cmd::launch::Args),
    Discover(cmd::discover::Args),
    Inspect(cmd::inspect::Args),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    gstreamer::init()?;

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Daemon(a)   => cmd::daemon::run(a).await,
        Cmd::Play(a)     => cmd::play::run(a).await,
        Cmd::Launch(a)   => cmd::launch::run(a).await,
        Cmd::Discover(a) => cmd::discover::run(a).await,
        Cmd::Inspect(a)  => cmd::inspect::run(a).await,
    }
}
```

`crates/gstpop-cli/src/cmd/mod.rs`:

```rust
pub mod daemon;
pub mod discover;
pub mod inspect;
pub mod launch;
pub mod play;

use clap::Args;

#[derive(Args, Clone, Debug)]
pub struct ServerArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: String,
    #[arg(long, default_value_t = 9000)]
    pub port: u16,
    #[arg(long)]
    pub api_key: Option<String>,
    #[arg(long)]
    pub allowed_origin: Vec<String>,
}

impl ServerArgs {
    pub fn into_config(self) -> gstpop_runtime::EmbeddedConfig {
        gstpop_runtime::EmbeddedConfig {
            bind: self.bind,
            port: self.port,
            api_key: self.api_key,
            allowed_origins: self.allowed_origin,
        }
    }
}
```

## `daemon` subcommand

`crates/gstpop-cli/src/cmd/daemon.rs`:

```rust
use clap::Args;

#[derive(Args, Debug)]
pub struct Args {
    #[command(flatten)]
    server: super::ServerArgs,
}

pub async fn run(args: Args) -> anyhow::Result<()> {
    let status = gstpop_runtime::start_embedded_with_config(args.server.into_config()).await;
    if !matches!(status.state, gstpop_runtime::EmbeddedState::Running) {
        anyhow::bail!(
            "failed to start daemon: state={:?}, error={:?}",
            status.state,
            status.last_error,
        );
    }
    tracing::info!("gstpop daemon listening on {}:{}", status.bind, status.port);
    crate::signal::wait_for_shutdown().await;
    gstpop_runtime::stop_embedded().await;
    Ok(())
}
```

## `play` subcommand

`crates/gstpop-cli/src/cmd/play.rs`:

```rust
use clap::Args;

#[derive(Args, Debug)]
pub struct Args {
    pub input: String,
    #[arg(long)] pub video_sink: Option<String>,
    #[arg(long)] pub audio_sink: Option<String>,
    #[arg(long)] pub playbin2: bool,
}

pub async fn run(a: Args) -> anyhow::Result<()> {
    let desc = gstpop_runtime::build_playbin_description(
        &a.input,
        None,
        a.video_sink.as_deref(),
        a.audio_sink.as_deref(),
        a.playbin2,
    )?;
    tracing::info!("playing: {desc}");
    // delegate to vendor/gstpop's blocking playback helper, or build a
    // PipelineManager directly. See vendor/gstpop/src/playback.rs.
    gstpop::playback::play_blocking(&desc)?;
    Ok(())
}
```

## `discover` and `inspect`

```rust
// cmd/discover.rs
use clap::Args;
#[derive(Args, Debug)]
pub struct Args { pub input: String }
pub async fn run(a: Args) -> anyhow::Result<()> {
    let info = gstpop_runtime::discover(&a.input, None, None).await?;
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "uri": info.uri,
        "duration_ms": info.duration.map(|d| d.as_millis() as u64),
        "seekable": info.seekable,
        "video_streams": info.video_streams,
        "audio_streams": info.audio_streams,
        "subtitle_streams": info.subtitle_streams,
        "tags": info.tags,
    }))?);
    Ok(())
}

// cmd/inspect.rs
use clap::Args;
#[derive(Args, Debug)]
pub struct Args { pub element: Option<String> }
pub async fn run(a: Args) -> anyhow::Result<()> {
    match a.element {
        None => for e in gstpop_runtime::list_elements()? {
            println!("{:>6} {:<30} {}", e.rank, e.name, e.long_name);
        },
        Some(name) => match gstpop_runtime::inspect_element(&name)? {
            Some(t) => println!("{t}"),
            None => anyhow::bail!("no such element: {name}"),
        },
    }
    Ok(())
}
```

## CLI tests

`crates/gstpop-cli/tests/cli.rs`:

```rust
use std::process::Command;

fn bin() -> &'static str { env!("CARGO_BIN_EXE_gstpop") }

#[test]
fn help_works() {
    let out = Command::new(bin()).arg("--help").output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    for sub in ["daemon", "play", "launch", "discover", "inspect"] {
        assert!(s.contains(sub), "missing subcommand in --help: {sub}");
    }
}

#[test]
fn daemon_help_lists_server_flags() {
    let out = Command::new(bin()).args(["daemon", "--help"]).output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    for flag in ["--bind", "--port", "--api-key", "--allowed-origin"] {
        assert!(s.contains(flag), "missing flag: {flag}");
    }
}
```

## Verification

```bash
cargo build -p gstpop-cli
cargo test  -p gstpop-cli
cargo run   -p gstpop-cli -- --help
cargo run   -p gstpop-cli -- daemon --port 9100 &
sleep 1
cargo run   -p gstpop-cli -- discover /path/to/local.mp4
kill %1
```

## Done when

- `cargo run -p gstpop-cli -- --help` lists all five subcommands.
- `daemon` starts an embedded server and exits cleanly on Ctrl-C.
- `play`, `discover`, `inspect` work against a local fixture.
- `gstpop-runtime` does **not** depend on `clap` or `tracing-subscriber`.
