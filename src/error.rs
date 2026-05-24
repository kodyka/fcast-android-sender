//! Top-level error types surfaced to the UI.

use std::fmt;

/// Top-level error categories surfaced to the UI.
#[derive(Clone, Debug)]
pub enum AppError {
    /// The service is not running or unreachable.
    ServiceUnavailable { service: String, detail: String },
    /// A media pipeline operation failed.
    PipelineError { node_id: String, detail: String },
    /// SRT source connection or streaming error.
    SrtError { slot_id: String, detail: String },
    /// Overlay image loading or composition error.
    OverlayError { overlay_id: String, detail: String },
    /// Configuration load/save failure.
    ConfigError { path: String, detail: String },
    /// Network or connectivity problem.
    NetworkError { detail: String },
    /// Catch-all for unexpected errors.
    Internal { detail: String },
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
            Self::ServiceUnavailable { .. } | Self::SrtError { .. } | Self::NetworkError { .. }
        )
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal {
            detail: format!("{err:#}"),
        }
    }
}

/// Push an `AppError` to the UI banner with severity inferred from the variant.
pub fn push_app_error(weak: &slint::Weak<crate::MainWindow>, err: &AppError) {
    use slint::ComponentHandle;

    let message = err.user_message();
    let severity = match err {
        AppError::ServiceUnavailable { .. }
        | AppError::SrtError { .. }
        | AppError::NetworkError { .. } => crate::BannerSeverity::Warning,
        AppError::PipelineError { .. }
        | AppError::OverlayError { .. }
        | AppError::ConfigError { .. }
        | AppError::Internal { .. } => crate::BannerSeverity::Error,
    };
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<crate::Bridge>();
        bridge.set_banner_message(message.into());
        bridge.set_banner_severity(severity);
        bridge.set_banner_visible(true);
    });
}

/// Best-effort recovery for retriable errors. Returns true when a recovery
/// action was dispatched, false when the caller should surface the error.
pub fn try_recover(
    err: &AppError,
    manager: &dyn crate::service::ServiceManager,
) -> bool {
    if !err.is_retriable() {
        return false;
    }
    let mgr_name = manager.name().to_string();
    tokio::spawn({
        let mgr = manager.options();
        async move {
            let _ = mgr;
            tracing::info!(service = %mgr_name, "try_recover: dispatching auto-restart");
            if let Some(handle) = crate::service::registry::lookup(&mgr_name) {
                if let Err(e) = handle.start().await {
                    tracing::warn!(error = %e, service = %mgr_name, "auto-recovery start failed");
                }
            }
        }
    });
    true
}
