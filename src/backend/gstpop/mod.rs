pub mod backend;
pub mod client;
mod embedded;
pub mod protocol;
#[cfg(test)]
mod protocol_tests;

pub use backend::GstPopBackend;
