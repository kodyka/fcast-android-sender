//! JNI-backed SecretStore — forwards every call into Kotlin.

use jni::objects::{JByteArray, JString, JValue};
use jni::JavaVM;
use std::sync::Arc;

use crate::secret::{SecretBytes, SecretError, SecretStore};

const BRIDGE_CLASS: &str = "org/fcast/android/sender/data/SecretStoreBridge";

pub struct JniSecretStore {
    vm: Arc<JavaVM>,
}

impl JniSecretStore {
    pub fn new(vm: Arc<JavaVM>) -> Self { Self { vm } }
}

impl SecretStore for JniSecretStore {
    fn get(&self, alias: &str) -> Result<SecretBytes, SecretError> {
        let mut env = self.vm.attach_current_thread()
            .map_err(|e| SecretError::Backend(format!("attach_current_thread: {e}")))?;

        let class = env.find_class(BRIDGE_CLASS)
            .map_err(|e| SecretError::Backend(format!("find_class: {e}")))?;
        let alias_j: JString = env.new_string(alias)
            .map_err(|e| SecretError::Backend(format!("new_string: {e}")))?;

        let res = env.call_static_method(
            class,
            "jniGet",
            "(Ljava/lang/String;)[B",
            &[JValue::Object(&alias_j.into())],
        ).map_err(|e| SecretError::Backend(format!("call_static_method: {e}")))?;

        let bytes_obj = res.l().map_err(|e| SecretError::Backend(format!("not an object: {e}")))?;
        if bytes_obj.is_null() {
            return Err(SecretError::NotFound(alias.to_owned()));
        }
        let bytes_j: JByteArray = bytes_obj.into();
        let len = env.get_array_length(&bytes_j)
            .map_err(|e| SecretError::Backend(format!("get_array_length: {e}")))? as usize;
        let mut buf = vec![0i8; len];
        env.get_byte_array_region(&bytes_j, 0, &mut buf)
            .map_err(|e| SecretError::Backend(format!("get_byte_array_region: {e}")))?;
        let buf_u8: Vec<u8> = buf.into_iter().map(|b| b as u8).collect();
        Ok(SecretBytes::new(buf_u8))
    }

    fn put(&self, alias: &str, value: &[u8]) -> Result<(), SecretError> {
        let mut env = self.vm.attach_current_thread()
            .map_err(|e| SecretError::Backend(format!("attach_current_thread: {e}")))?;
        let class = env.find_class(BRIDGE_CLASS)
            .map_err(|e| SecretError::Backend(format!("find_class: {e}")))?;
        let alias_j = env.new_string(alias)
            .map_err(|e| SecretError::Backend(format!("new_string: {e}")))?;
        let bytes_j = env.byte_array_from_slice(value)
            .map_err(|e| SecretError::Backend(format!("byte_array_from_slice: {e}")))?;
        env.call_static_method(
            class,
            "jniPut",
            "(Ljava/lang/String;[B)V",
            &[JValue::Object(&alias_j.into()), JValue::Object(&bytes_j.into())],
        ).map_err(|e| SecretError::Backend(format!("call_static_method jniPut: {e}")))?;
        Ok(())
    }

    fn delete(&self, alias: &str) -> Result<(), SecretError> {
        let mut env = self.vm.attach_current_thread()
            .map_err(|e| SecretError::Backend(format!("attach: {e}")))?;
        let class = env.find_class(BRIDGE_CLASS)
            .map_err(|e| SecretError::Backend(format!("find_class: {e}")))?;
        let alias_j = env.new_string(alias)
            .map_err(|e| SecretError::Backend(format!("new_string: {e}")))?;
        env.call_static_method(
            class,
            "jniDelete",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&alias_j.into())],
        ).map_err(|e| SecretError::Backend(format!("call_static_method jniDelete: {e}")))?;
        Ok(())
    }
}
