pub mod backend;
pub mod client;
pub mod protocol;
#[cfg(test)]
mod protocol_tests;

pub use backend::GstPopBackend;
