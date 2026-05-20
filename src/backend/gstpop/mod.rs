pub mod backend;
pub mod client;
pub mod embedded;
pub mod protocol;
pub mod service;
#[cfg(test)]
mod protocol_tests;

pub use backend::GstPopBackend;
