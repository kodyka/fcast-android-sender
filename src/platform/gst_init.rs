//! GStreamer initialisation helpers.

#[cfg(target_os = "android")]
pub(crate) fn ensure_gstreamer_initialized() -> std::result::Result<(), String> {
    use std::sync::OnceLock;

    static GST_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    GST_INIT
        .get_or_init(|| gst::init().map_err(|err| format!("Failed to initialize GStreamer: {err}")))
        .clone()
}

#[cfg(not(target_os = "android"))]
pub(crate) fn ensure_gstreamer_initialized() -> std::result::Result<(), String> {
    gst::init().map_err(|err| format!("Failed to initialize GStreamer: {err}"))
}
