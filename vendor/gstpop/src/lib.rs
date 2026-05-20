#[cfg(target_os = "linux")]
pub mod dbus;
pub mod error;
pub mod gst;
pub mod playback;
pub mod server;
pub mod websocket;

pub use error::{GstpopError, Result};
pub use gst::{
    create_event_channel, Pipeline, PipelineEvent, PipelineInfo, PipelineManager, PipelineState,
};
