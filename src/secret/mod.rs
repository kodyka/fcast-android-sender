//! Secret store — opaque alias-to-bytes mapping owned by the App context.
//!
//! Production implementation forwards to the Android Keystore via JNI.
//! Tests use [InMemorySecretStore].

use std::sync::Mutex;
use std::collections::HashMap;

/// Errors returned by the secret store.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("unknown alias: {0}")]
    NotFound(String),

    #[error("backend error: {0}")]
    Backend(String),
}

/// Owning byte buffer that zeroises on Drop.
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn new(v: Vec<u8>) -> Self { Self(v) }
    pub fn as_slice(&self) -> &[u8] { &self.0 }
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.0)
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        for b in &mut self.0 { *b = 0; }
    }
}

impl std::fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretBytes(<{} bytes>)", self.0.len())
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, alias: &str) -> Result<SecretBytes, SecretError>;
    fn put(&self, alias: &str, value: &[u8]) -> Result<(), SecretError>;
    fn delete(&self, alias: &str) -> Result<(), SecretError>;
}

/// In-memory implementation; used by Rust unit tests and as a fallback when
/// the host platform has no secret store available.
pub struct InMemorySecretStore {
    inner: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }
}

impl Default for InMemorySecretStore {
    fn default() -> Self { Self::new() }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, alias: &str) -> Result<SecretBytes, SecretError> {
        self.inner
            .lock()
            .unwrap()
            .get(alias)
            .cloned()
            .map(SecretBytes::new)
            .ok_or_else(|| SecretError::NotFound(alias.to_owned()))
    }

    fn put(&self, alias: &str, value: &[u8]) -> Result<(), SecretError> {
        self.inner.lock().unwrap().insert(alias.to_owned(), value.to_vec());
        Ok(())
    }

    fn delete(&self, alias: &str) -> Result<(), SecretError> {
        self.inner.lock().unwrap().remove(alias);
        Ok(())
    }
}

/// Convenience accessor — returns the secret as UTF-8 (typical for API keys).
pub fn resolve_secret_str(alias: &str) -> Result<String, SecretError> {
    let bytes = crate::app::app().secrets().get(alias)?;
    let s = bytes
        .as_str()
        .map_err(|e| SecretError::Backend(format!("not utf-8: {e}")))?;
    Ok(s.to_owned())
}

/// Convenience accessor for non-text secrets.
pub fn resolve_secret_bytes(alias: &str) -> Result<SecretBytes, SecretError> {
    crate::app::app().secrets().get(alias)
}

#[cfg(target_os = "android")]
pub mod jni;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = InMemorySecretStore::new();
        s.put("a", b"hello").unwrap();
        let got = s.get("a").unwrap();
        assert_eq!(got.as_slice(), b"hello");
    }

    #[test]
    fn not_found_is_distinct() {
        let s = InMemorySecretStore::new();
        match s.get("a") {
            Err(SecretError::NotFound(name)) => assert_eq!(name, "a"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn debug_never_leaks_value() {
        let s = SecretBytes::new(b"hello world".to_vec());
        let debug = format!("{s:?}");
        assert!(!debug.contains("hello"));
    }
}
