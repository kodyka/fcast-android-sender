//! Shared helpers used by every jni_bridge::* module.
//! Extracted from src/lib.rs as part of refactor step 07.

#[cfg(target_os = "android")]
use anyhow::{bail, Result};
#[cfg(target_os = "android")]
use gst::prelude::{BufferPoolExt, BufferPoolExtManual};
#[cfg(target_os = "android")]
use gst_video::{VideoColorimetry, VideoFrameExt};
#[cfg(target_os = "android")]
use jni::{
    objects::{JByteBuffer, JObject, JString},
    JavaVM,
};
#[cfg(target_os = "android")]
use once_cell::sync::OnceCell;
#[cfg(target_os = "android")]
use slint::ComponentHandle;
#[cfg(target_os = "android")]
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::Arc;
#[cfg(target_os = "android")]
use tracing::{error, warn};

use crate::platform::platform_app::PlatformApp;
#[cfg(target_os = "android")]
use mcore::Event;

#[cfg(target_os = "android")]
pub(crate) fn jstring_to_string<'local>(
    env: &mut jni::JNIEnv<'local>,
    s: &JString<'local>,
) -> Result<String> {
    Ok(env.get_string(s)?.to_string_lossy().to_string())
}

#[cfg(target_os = "android")]
static VM: OnceCell<Arc<JavaVM>> = OnceCell::new();

#[cfg(target_os = "android")]
pub(crate) fn init_vm(vm: JavaVM) -> Arc<JavaVM> {
    let vm = Arc::new(vm);
    if VM.set(vm.clone()).is_err() {
        warn!("init_vm called twice; keeping the first JavaVM handle");
    }
    VM.get()
        .expect("JavaVM missing immediately after init_vm")
        .clone()
}

#[cfg(target_os = "android")]
pub(crate) fn vm() -> Arc<JavaVM> {
    VM.get()
        .expect("JavaVM not initialised; call init_vm() from android_main")
        .clone()
}

#[derive(Debug)]
pub(crate) enum JavaMethod {
    StopCapture,
    ScanQr,
    FinishApp,
}

#[cfg(target_os = "android")]
pub(crate) fn call_java_method_no_args(app: &PlatformApp, method: JavaMethod) {
    let vm = vm();
    let ptr = app.activity_as_ptr() as *mut jni::sys::_jobject;
    assert!(!ptr.is_null(), "Activity ptr is null");
    // SAFETY: PlatformApp owns the Android activity handle for the lifetime of
    // the Slint Android runtime. This helper only creates a local wrapper for
    // the immediate call on the current UI callback.
    let activity = unsafe {
        JObject::from_raw(ptr)
    };

    let method_name = match method {
        JavaMethod::StopCapture => "stopCapture",
        JavaMethod::ScanQr => "scanQr",
        JavaMethod::FinishApp => "finishApp",
    };

    match vm.get_env() {
        Ok(mut env) => match env.call_method(activity, method_name, "()V", &[]) {
            Ok(_) => (),
            Err(err) => error!(?err, ?method, "Failed to call java method"),
        },
        Err(err) => error!(?err, "Failed to get env from VM"),
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn call_java_method_no_args(_app: &PlatformApp, _method: JavaMethod) {}

#[cfg(target_os = "android")]
pub(crate) fn handle_back_request(ui: &crate::MainWindow, app: Option<&PlatformApp>) {
    let bridge = ui.global::<crate::Bridge>();

    if ui.global::<crate::PanelBridge>().get_active() != crate::Panel::None {
        ui.global::<crate::PanelBridge>().invoke_pop();
        return;
    }

    if bridge.get_lifecycle() != crate::LifecycleMode::Normal {
        bridge.set_lifecycle(crate::LifecycleMode::Normal);
        return;
    }

    match bridge.get_app_state() {
        crate::AppState::Disconnected => {
            if let Some(app) = app {
                call_java_method_no_args(app, JavaMethod::FinishApp);
            } else {
                warn!("Ignoring back press in disconnected state without Android app handle");
            }
        }
        crate::AppState::Connecting | crate::AppState::SelectingSettings => {
            bridge.invoke_change_state(crate::AppState::Disconnected);
        }
        crate::AppState::WaitingForMedia | crate::AppState::Casting => {
            if let Err(err) = crate::GLOB_EVENT_CHAN
                .0
                .send(Event::EndSession { disconnect: true })
            {
                error!(?err, "Failed to send back-requested end-session event");
            }
        }
    }
}

#[cfg(target_os = "android")]
pub(crate) fn resolve_android_files_dir(app: &PlatformApp) -> Result<PathBuf> {
    let vm = vm();
    let ptr = app.activity_as_ptr() as *mut jni::sys::_jobject;
    assert!(!ptr.is_null(), "Activity ptr is null");
    // SAFETY: PlatformApp exposes the live Activity object owned by the Slint
    // Android runtime. The wrapper is used only while resolving the files dir
    // on the current thread and is not retained.
    let activity = unsafe {
        JObject::from_raw(ptr)
    };

    let mut env = vm.get_env()?;
    let files_dir = env
        .call_method(&activity, "getFilesDir", "()Ljava/io/File;", &[])?
        .l()?;
    let absolute_path = env
        .call_method(files_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
        .l()?;
    let absolute_path = JString::from(absolute_path);
    let absolute_path = env
        .get_string(&absolute_path)?
        .to_string_lossy()
        .to_string();

    Ok(PathBuf::from(absolute_path))
}

#[cfg(target_os = "android")]
pub(crate) fn process_frame<'local>(
    env: jni::JNIEnv<'local>,
    width: jni::sys::jint,
    height: jni::sys::jint,
    buffer_y: JByteBuffer<'local>,
    buffer_u: JByteBuffer<'local>,
    buffer_v: JByteBuffer<'local>,
) -> Result<()> {
    let width = width as usize;
    let height = height as usize;

    fn buffer_as_slice<'local>(
        env: &jni::JNIEnv<'local>,
        buffer: &JByteBuffer<'local>,
        size: usize,
    ) -> Result<&'local [u8]> {
        let buffer_cap = match env.get_direct_buffer_capacity(&buffer) {
            Ok(cap) => cap,
            Err(err) => {
                bail!("Failed to get capacity of the byte buffer: {err}");
            }
        };

        if buffer_cap < size {
            bail!("buffer_cap < size: {buffer_cap} < {size}");
        }

        let buffer_ptr = match env.get_direct_buffer_address(&buffer) {
            Ok(ptr) => {
                assert!(!ptr.is_null());
                ptr
            }
            Err(err) => {
                bail!("Failed to get buffer address: {err}");
            }
        };

        // SAFETY: get_direct_buffer_address/capacity came from the same live
        // DirectByteBuffer local reference, and callers pass the buffer through
        // JNI for the duration of this native frame callback.
        unsafe { Ok(std::slice::from_raw_parts(buffer_ptr, buffer_cap)) }
    }

    let slice_y = buffer_as_slice(&env, &buffer_y, width * height)?;
    let slice_u = buffer_as_slice(&env, &buffer_u, (width / 2) * (height / 2))?;
    let slice_v = buffer_as_slice(&env, &buffer_v, (width / 2) * (height / 2))?;

    let info = match gst_video::VideoInfo::builder(
        gst_video::VideoFormat::I420,
        width as u32,
        height as u32,
    )
    .colorimetry(&VideoColorimetry::new(
        gst_video::VideoColorRange::Range0_255,
        gst_video::VideoColorMatrix::Bt709,
        gst_video::VideoTransferFunction::Bt709,
        gst_video::VideoColorPrimaries::Bt709,
    ))
    .build()
    {
        Ok(info) => info,
        Err(err) => {
            bail!("Failed to crate video info: {err}");
        }
    };

    let new_caps = match info.to_caps() {
        Ok(caps) => caps,
        Err(err) => {
            bail!("Failed to create caps from video info: {err}");
        }
    };

    fn init_frame_pool(
        pool: &gst_video::VideoBufferPool,
        mut old_config: gst::BufferPoolConfig,
        new_caps: &gst::Caps,
        frame_size: u32,
    ) -> Result<()> {
        pool.set_config({
            old_config.set_params(Some(&new_caps), frame_size, 1, 30);
            old_config
        })?;
        pool.set_active(true)?;
        Ok(())
    }

    let mut frame_pool = crate::FRAME_POOL.lock();
    let frame_size = width * height + 2 * ((width / 2) * (height / 2));
    let needs_reconfigure = if !frame_pool.is_active() {
        true
    } else {
        match frame_pool.config().params() {
            Some((caps, size, _, _)) => {
                caps.as_ref() != Some(&new_caps) || size != frame_size as u32
            }
            None => true,
        }
    };
    if needs_reconfigure {
        let old_config = frame_pool.config();
        if frame_pool.is_active() {
            let _ = frame_pool.set_active(false);
        }
        init_frame_pool(&frame_pool, old_config, &new_caps, frame_size as u32)?;
    }

    let buffer = match frame_pool.acquire_buffer(None) {
        Ok(buffer) => buffer,
        Err(err) => {
            bail!("Failed to acquire buffer from pool: {err}");
        }
    };
    let Ok(mut vframe) = gst_video::VideoFrame::from_buffer_writable(buffer, &info) else {
        bail!("Failed to crate VideoFrame from buffer");
    };

    fn copy(
        vframe: &mut gst_video::VideoFrame<gst_video::video_frame::Writable>,
        plane_idx: u32,
        src_plane: &[u8],
    ) -> Result<()> {
        let dest_y_stride = *vframe
            .plane_stride()
            .get(plane_idx as usize)
            .ok_or(anyhow::anyhow!("Could not get plane stride"))?
            as usize;
        let dest_y = vframe.plane_data_mut(plane_idx)?;
        for (dest, src) in dest_y
            .chunks_exact_mut(dest_y_stride)
            .zip(src_plane.chunks_exact(dest_y_stride))
        {
            dest[..dest_y_stride].copy_from_slice(&src[..dest_y_stride]);
        }

        Ok(())
    }

    copy(&mut vframe, 0, slice_y)?;
    copy(&mut vframe, 1, slice_u)?;
    copy(&mut vframe, 2, slice_v)?;

    let mut frame = crate::FRAME_PAIR.frame.lock();
    *frame = Some(vframe);
    crate::FRAME_PAIR.cond.notify_one();

    Ok(())
}
