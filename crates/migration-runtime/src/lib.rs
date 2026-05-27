//! migration-runtime — extracted from `android-sender`.
//!
//! This crate hosts the in-process FCast media-graph runtime. The Android
//! foreground service and the desktop probe both drive it through
//! extracted runtime entry points.
//!
//! Currently empty; populated by subsequent extraction steps.

/// Crate identity probe. Removed once extracted runtime modules land.
pub const CRATE_NAME: &str = "migration-runtime";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn crate_name_set() {
        assert_eq!(CRATE_NAME, "migration-runtime");
    }
}
