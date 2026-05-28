//! JNI bridge — symbols called from FCastDiscoveryListener.

#[cfg(target_os = "android")]
use std::net::Ipv6Addr;

#[cfg(target_os = "android")]
use jni::{
    objects::{JByteBuffer, JClass, JObject, JString},
    JNIEnv,
};
#[cfg(target_os = "android")]
use mcore::Event;
#[cfg(target_os = "android")]
use tracing::{debug, error};

#[cfg(target_os = "android")]
use crate::jni_bridge::helpers::jstring_to_string;

#[cfg(target_os = "android")]
pub fn service_found<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
    addrs: JObject<'local>,
    port: jni::sys::jint,
) {
    let name = match jstring_to_string(&mut env, &name) {
        Ok(name) => name,
        Err(err) => {
            error!(?err, "Failed to convert jstring to string");
            return;
        }
    };
    let port = port as u16;
    let addrs = match jni::objects::JList::from_env(&mut env, &addrs) {
        Ok(addrs) => addrs,
        Err(err) => {
            error!(?err, "Failed to get address list from env");
            return;
        }
    };
    let mut ip_addrs = Vec::<fcast_sender_sdk::IpAddr>::new();
    let n_addrs = match addrs.size(&mut env) {
        Ok(n) => n,
        Err(err) => {
            error!(?err, "Failed to get JList size");
            return;
        }
    };
    for i in 0..n_addrs {
        let Ok(Some(addr)) = addrs.get(&mut env, i) else {
            continue;
        };
        let buffer = unsafe { JByteBuffer::from_raw(*addr) };

        let buffer_cap = match env.get_direct_buffer_capacity(&buffer) {
            Ok(cap) => cap,
            Err(err) => {
                error!(?err, "Failed to get capacity of the byte buffer");
                continue;
            }
        };

        debug!(buffer_cap);

        let buffer_ptr = match env.get_direct_buffer_address(&buffer) {
            Ok(ptr) => {
                assert!(!ptr.is_null());
                ptr
            }
            Err(err) => {
                error!(?err, "Failed to get buffer address");
                continue;
            }
        };

        let buffer_slice: &[u8] = unsafe { std::slice::from_raw_parts(buffer_ptr, buffer_cap) };

        ip_addrs.push(match buffer_slice.len() {
            4 => fcast_sender_sdk::IpAddr::v4(
                buffer_slice[0],
                buffer_slice[1],
                buffer_slice[2],
                buffer_slice[3],
            ),
            20 => {
                let mut addr_slice = [0; 16];
                for i in 0..addr_slice.len() {
                    addr_slice[i] = buffer_slice[i];
                }
                let addr = Ipv6Addr::from(addr_slice);
                let scope_id_slice = &buffer_slice[16..20];
                let this_scope_id = i32::from_le_bytes([
                    scope_id_slice[0],
                    scope_id_slice[1],
                    scope_id_slice[2],
                    scope_id_slice[3],
                ]) as u32;
                let mut ip = fcast_sender_sdk::IpAddr::from(std::net::IpAddr::V6(addr));
                match &mut ip {
                    fcast_sender_sdk::IpAddr::V6 { scope_id, .. } => *scope_id = this_scope_id,
                    _ => (),
                }
                ip
            }
            len => {
                error!(len, "Invalid address buffer length");
                continue;
            }
        });
    }

    let device_info = fcast_sender_sdk::device::DeviceInfo::fcast(name, ip_addrs, port);
    debug!(?device_info, "Found device");

    if let Err(err) = crate::GLOB_EVENT_CHAN
        .0
        .send(Event::DeviceAvailable(device_info))
    {
        error!(?err, "Failed to send device available event");
    }
}

#[cfg(target_os = "android")]
pub fn service_lost<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>, name: JString<'local>) {
    match jstring_to_string(&mut env, &name) {
        Ok(name) => {
            if let Err(err) = crate::GLOB_EVENT_CHAN.0.send(Event::DeviceRemoved(name)) {
                error!(?err, "Failed to send device removed event");
            }
        }
        Err(err) => error!(?err, "Failed to convert jstring to string"),
    }
}
