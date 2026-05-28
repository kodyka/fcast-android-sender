//! In-process gst-pop daemon host + JSON-RPC client.
//!
//! Extracted from `android-sender`. See
//! `docs/gstpop-runtime-crate-extraction/` for the extraction plan and
//! `docs/gstpop-service-architecture.md` for the runtime architecture.

pub mod client;
pub mod embedded;
pub mod protocol;

#[cfg(test)]
mod protocol_tests;

pub use client::GstPopClient;
pub use embedded::{
    embedded_status, is_localhost, start_embedded, stop_embedded, url_port, EmbeddedState,
    EmbeddedStatus,
};
pub use protocol::{classify, ClassifiedFrame, Event, Request, Response};
