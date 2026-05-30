# Step 9 — Typed protocol enums

**Phase:** 2 — Android polish
**Priority:** medium
**Depends on:** Step 3
**Unblocks:** clearer UI state handling

## Goal

Replace the loose string-based pipeline-state and event-kind fields in
`gstpop-runtime::protocol` with typed enums that mirror
`vendor/gstpop/src/gst/event.rs`. Keep an explicit forward-compatibility
escape hatch so unknown variants from a newer server don't crash the client.

## Files touched

- `crates/gstpop-runtime/src/protocol.rs`
- `crates/gstpop-runtime/src/typed_client.rs` (`PipelineSummary::state` → typed enum)
- `crates/gstpop-runtime/src/lib.rs` (re-exports)

## Implementation

### 1. Add typed enums to `protocol.rs`

Append:

```rust
use serde::{Deserialize, Serialize};

/// Mirror of `gstpop::gst::event::PipelineState`. Unknown server values fall
/// through to `Other(String)` so clients survive server upgrades.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    Null,
    Ready,
    Paused,
    Playing,
    #[serde(other)]
    Other,
}

impl PipelineState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Ready => "ready",
            Self::Paused => "paused",
            Self::Playing => "playing",
            Self::Other => "other",
        }
    }
}

/// Mirror of the daemon's `PipelineEvent` discriminant. Kept opaque on
/// payload to avoid a second migration step if the server adds fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineEventKind {
    StateChanged,
    Position,
    Eos,
    Error,
    Warning,
    Info,
    StreamStart,
    AsyncDone,
    #[serde(other)]
    Other,
}
```

> `#[serde(other)]` requires the variant to be unit-only (`Other` not
> `Other(String)`). If you need to preserve the raw string, deserialise into
> `String` first and convert manually — see the helper below.

### 2. Optional: preserve unknown strings

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineStateExt {
    Known(PipelineState),
    Unknown(String),
}

impl<'de> Deserialize<'de> for PipelineStateExt {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let known = match s.as_str() {
            "null"    => Some(PipelineState::Null),
            "ready"   => Some(PipelineState::Ready),
            "paused"  => Some(PipelineState::Paused),
            "playing" => Some(PipelineState::Playing),
            _ => None,
        };
        Ok(match known {
            Some(k) => PipelineStateExt::Known(k),
            None    => PipelineStateExt::Unknown(s),
        })
    }
}

impl serde::Serialize for PipelineStateExt {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Known(k) => s.serialize_str(k.as_str()),
            Self::Unknown(raw) => s.serialize_str(raw),
        }
    }
}
```

Use `PipelineStateExt` in client-facing structs when you want to log unknown
states without losing the original string.

### 3. Tighten `PipelineSummary`

Edit `crates/gstpop-runtime/src/typed_client.rs`:

```rust
use crate::protocol::PipelineStateExt;

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineSummary {
    pub id: String,
    pub description: String,
    pub state: PipelineStateExt,         // was: String
    #[serde(default)]
    pub streaming: bool,
}
```

### 4. Re-export

```rust
// crates/gstpop-runtime/src/lib.rs
pub use protocol::{
    classify, ClassifiedFrame, Event, PipelineEventKind, PipelineState,
    PipelineStateExt, Request, Response,
};
```

## Tests

Append to `protocol.rs` (or `protocol_tests.rs`):

```rust
#[cfg(test)]
mod typed_state_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn known_states_round_trip() {
        for raw in ["null", "ready", "paused", "playing"] {
            let parsed: PipelineState = serde_json::from_value(json!(raw)).unwrap();
            assert_eq!(parsed.as_str(), raw);
        }
    }

    #[test]
    fn unknown_state_falls_through() {
        let parsed: PipelineState =
            serde_json::from_value(json!("future_state")).unwrap();
        assert!(matches!(parsed, PipelineState::Other));
    }

    #[test]
    fn ext_preserves_unknown() {
        let parsed: PipelineStateExt =
            serde_json::from_value(json!("future_state")).unwrap();
        match parsed {
            PipelineStateExt::Unknown(s) => assert_eq!(s, "future_state"),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn event_kinds() {
        let parsed: PipelineEventKind =
            serde_json::from_value(json!("state_changed")).unwrap();
        assert_eq!(parsed, PipelineEventKind::StateChanged);

        let unknown: PipelineEventKind =
            serde_json::from_value(json!("new_event")).unwrap();
        assert_eq!(unknown, PipelineEventKind::Other);
    }
}
```

## Migration note

Callers that pattern-matched on `pipeline.state` as a `String` need updating.
The mechanical conversion:

```rust
// before
if summary.state == "playing" { ... }

// after
matches!(summary.state, PipelineStateExt::Known(PipelineState::Playing))
```

## Verification

```bash
cargo build -p gstpop-runtime --features typed-client
cargo test  -p gstpop-runtime --features typed-client --lib protocol
```

## Done when

- `PipelineState` and `PipelineEventKind` enums exist and accept unknown
  variants without error.
- `PipelineSummary.state` is typed.
- Existing integration tests from Step 5 still pass.
