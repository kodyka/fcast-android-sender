# STEP 14 — Error Handling Standardisation

**Phase:** 5 (Codebase Restructuring)
**Cross-cutting**

---

## Goal

Create unified error types across service boundaries, with user-friendly
messages for UI display and structured recovery mechanisms.

---

## 1. Define a crate-level error enum

```rust
// src/error.rs  (new file)

use std::fmt;

/// Top-level error categories surfaced to the UI.
#[derive(Clone, Debug)]
pub enum AppError {
    /// The service is not running or unreachable.
    ServiceUnavailable {
        service: String,
        detail: String,
    },
    /// A media pipeline operation failed.
    PipelineError {
        node_id: String,
        detail: String,
    },
    /// SRT source connection or streaming error.
    SrtError {
        slot_id: String,
        detail: String,
    },
    /// Overlay image loading or composition error.
    OverlayError {
        overlay_id: String,
        detail: String,
    },
    /// Configuration load/save failure.
    ConfigError {
        path: String,
        detail: String,
    },
    /// Network or connectivity problem.
    NetworkError {
        detail: String,
    },
    /// Catch-all for unexpected errors.
    Internal {
        detail: String,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServiceUnavailable { service, detail } => {
                write!(f, "{service} service is not available: {detail}")
            }
            Self::PipelineError { node_id, detail } => {
                write!(f, "Pipeline error (node {node_id}): {detail}")
            }
            Self::SrtError { slot_id, detail } => {
                write!(f, "SRT source {slot_id}: {detail}")
            }
            Self::OverlayError { overlay_id, detail } => {
                write!(f, "Overlay {overlay_id}: {detail}")
            }
            Self::ConfigError { path, detail } => {
                write!(f, "Config {path}: {detail}")
            }
            Self::NetworkError { detail } => {
                write!(f, "Network: {detail}")
            }
            Self::Internal { detail } => {
                write!(f, "Internal error: {detail}")
            }
        }
    }
}

impl std::error::Error for AppError {}
```

## 2. User-friendly message extraction

```rust
impl AppError {
    /// Short message suitable for a UI toast / banner.
    pub fn user_message(&self) -> String {
        match self {
            Self::ServiceUnavailable { service, .. } => {
                format!("{service} is not running. Start it from Service Configuration.")
            }
            Self::PipelineError { .. } => {
                "Media pipeline error. Check the debug log for details.".into()
            }
            Self::SrtError { slot_id, .. } => {
                format!("SRT source {slot_id} disconnected. Auto-reconnect will retry.")
            }
            Self::OverlayError { .. } => {
                "Failed to load overlay image. Check the file path.".into()
            }
            Self::ConfigError { .. } => {
                "Settings could not be saved. Check storage permissions.".into()
            }
            Self::NetworkError { .. } => {
                "Network error. Check your connection.".into()
            }
            Self::Internal { .. } => {
                "An unexpected error occurred. Please report this issue.".into()
            }
        }
    }

    /// Whether this error is recoverable by retrying.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::ServiceUnavailable { .. }
                | Self::SrtError { .. }
                | Self::NetworkError { .. }
        )
    }
}
```

## 3. Conversion from anyhow / other errors

```rust
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        // Attempt to downcast to a known error type.
        // If none match, wrap as Internal.
        Self::Internal {
            detail: format!("{err:#}"),
        }
    }
}
```

## 4. Push errors to the UI

```rust
// In any Rust async handler:

fn push_app_error(weak: &slint::Weak<MainWindow>, error: &AppError) {
    let message: slint::SharedString = error.user_message().into();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let banner = ui.global::<crate::BannerBridge>();
        banner.set_message(message);
        banner.set_severity(crate::BannerSeverity::Error);
        banner.set_visible(true);
    });
}
```

## 5. Recovery mechanisms

```rust
/// Attempt automatic recovery for retriable errors.
pub async fn try_recover(
    error: &AppError,
    service_manager: &dyn crate::service::ServiceManager,
) -> bool {
    match error {
        AppError::ServiceUnavailable { .. } => {
            // Try restarting the service.
            match service_manager.start().await {
                Ok(status) if status.running => {
                    tracing::info!("Service recovered after restart");
                    true
                }
                _ => false,
            }
        }
        AppError::SrtError { .. } => {
            // SRT reconnection is handled by SrtSourceManager's
            // health monitor (STEP 09). Signal it here.
            true
        }
        _ => false,
    }
}
```

## 6. Register the module

```rust
// src/lib.rs
pub mod error;
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `src/error.rs` with `AppError` enum | new file |
| 2 | Add `pub mod error;` to `lib.rs` | existing file |
| 3 | Replace `anyhow::Error` returns in service/lifecycle with `AppError` where user-facing | incremental |
| 4 | Wire `push_app_error` into lifecycle, SRT, and overlay error paths | multiple files |
| 5 | Verify `cargo check` passes | terminal |

---

## Notes

* `anyhow::Result` remains the internal plumbing type.  `AppError` is
  the boundary type at the Rust<->Slint interface — only errors that
  need to be shown to the user or trigger recovery are converted.
* The `user_message()` strings use `@tr()`-style phrasing so they can be
  migrated to Slint translation keys later.
