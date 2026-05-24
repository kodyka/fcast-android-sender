use crate::backend::persistence::StoredBackendConfig;
use crate::service::{ServiceManager, ServiceMode, ServiceOptions, ServiceStatus};

// ── Android impl ──────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android {
    use anyhow::{Context, Result};
    use jni::objects::{JObject, JValue};
    use jni::JNIEnv;

    /// `env.find_class()` uses the bootstrap ClassLoader on non-JVM-spawned threads
    /// and cannot see app classes. Use the activity's ClassLoader instead.
    pub(super) fn load_app_class<'e>(
        env: &mut JNIEnv<'e>,
        activity: &JObject<'_>,
        class_name: &str,
    ) -> Result<jni::objects::JClass<'e>> {
        let loader = env
            .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .context("getClassLoader")?
            .l()
            .context("getClassLoader result")?;
        let jname = env.new_string(class_name).context("new_string class name")?;
        let class = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&jname.into())],
            )
            .context("loadClass")?
            .l()
            .context("loadClass result")?;
        Ok(jni::objects::JClass::from(class))
    }
}

/// Ask the foreground GstPopService to start the daemon. Idempotent.
/// Respects `config.gstpop_service.enabled` — returns early if disabled.
#[cfg(target_os = "android")]
pub fn request_service_start(config: &StoredBackendConfig) -> anyhow::Result<()> {
    use anyhow::Context;
    use jni::objects::JValue;

    if let Some(ref svc) = config.gstpop_service {
        if !svc.enabled {
            tracing::info!("gst-pop service disabled by config; skipping start");
            return Ok(());
        }
    }

    let ctx = crate::android_context().context("android_context")?;
    let mut env = ctx.vm.attach_current_thread().context("attach_current_thread")?;
    let config_json = serde_json::to_string(config).context("serialize StoredBackendConfig")?;
    let jconfig = env.new_string(config_json).context("new_string(config)")?;
    let bridge = android::load_app_class(
        &mut env,
        &ctx.activity,
        "org.fcast.android.sender.GstPopServiceBridge",
    )
    .context("load GstPopServiceBridge")?;
    env.call_static_method(
        bridge,
        "start",
        "(Landroid/content/Context;Ljava/lang/String;)V",
        &[
            JValue::Object(&ctx.activity),
            JValue::Object(&jconfig.into()),
        ],
    )
    .context("call GstPopServiceBridge.start")?;
    Ok(())
}

/// Ask the foreground GstPopService to stop. Idempotent; safe if not running.
#[cfg(target_os = "android")]
pub fn request_service_stop() {
    use anyhow::Context;
    use jni::objects::JValue;

    if let Ok(ctx) = crate::android_context() {
        let _ = (|| -> anyhow::Result<()> {
            let mut env = ctx.vm.attach_current_thread().context("attach_current_thread")?;
            let bridge = android::load_app_class(
                &mut env,
                &ctx.activity,
                "org.fcast.android.sender.GstPopServiceBridge",
            )
            .context("load GstPopServiceBridge")?;
            env.call_static_method(
                bridge,
                "stop",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&ctx.activity)],
            )
            .context("call GstPopServiceBridge.stop")?;
            Ok(())
        })();
    }
}

// ── Non-Android stubs ─────────────────────────────────────────────────────────

#[cfg(not(target_os = "android"))]
pub fn request_service_start(_config: &StoredBackendConfig) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn request_service_stop() {}

// ── ServiceManager implementation ─────────────────────────────────────────────

pub struct GstPopServiceManager {
    options: parking_lot::RwLock<ServiceOptions>,
}

impl GstPopServiceManager {
    pub fn new(options: ServiceOptions) -> Self {
        Self {
            options: parking_lot::RwLock::new(options),
        }
    }
}

#[async_trait::async_trait]
impl ServiceManager for GstPopServiceManager {
    fn name(&self) -> &str {
        "gst-pop"
    }

    fn options(&self) -> ServiceOptions {
        self.options.read().clone()
    }

    fn set_options(&self, options: ServiceOptions) {
        *self.options.write() = options;
    }

    async fn start(&self) -> anyhow::Result<ServiceStatus> {
        let opts = self.options.read().clone();
        if !opts.enabled {
            return Ok(ServiceStatus {
                running: false,
                healthy: true,
                status_text: "gst-pop service disabled by configuration".into(),
                error_text: String::new(),
            });
        }

        match opts.mode {
            ServiceMode::Embedded => {
                let status = super::embedded::start_embedded(9000).await;
                Ok(ServiceStatus {
                    running: status.state == super::embedded::EmbeddedState::Running,
                    healthy: status.last_error.is_none(),
                    status_text: format!("embedded gst-pop on port {}", status.port),
                    error_text: status.last_error.unwrap_or_default(),
                })
            }
            ServiceMode::AndroidService => {
                let config = StoredBackendConfig::defaults();
                request_service_start(&config)?;
                Ok(ServiceStatus {
                    running: true,
                    healthy: true,
                    status_text: "Android service start requested".into(),
                    error_text: String::new(),
                })
            }
            ServiceMode::External => Ok(ServiceStatus {
                running: true,
                healthy: true,
                status_text: "using external gst-pop daemon".into(),
                error_text: String::new(),
            }),
        }
    }

    async fn stop(&self) -> anyhow::Result<ServiceStatus> {
        let opts = self.options.read().clone();
        match opts.mode {
            ServiceMode::Embedded => {
                let status = super::embedded::stop_embedded().await;
                Ok(ServiceStatus {
                    running: status.state == super::embedded::EmbeddedState::Running,
                    healthy: true,
                    status_text: "embedded gst-pop stopped".into(),
                    error_text: String::new(),
                })
            }
            ServiceMode::AndroidService => {
                request_service_stop();
                Ok(ServiceStatus {
                    running: false,
                    healthy: true,
                    status_text: "Android service stop requested".into(),
                    error_text: String::new(),
                })
            }
            ServiceMode::External => Ok(ServiceStatus {
                running: true,
                healthy: true,
                status_text: "external daemon — stop is a no-op".into(),
                error_text: String::new(),
            }),
        }
    }

    async fn status(&self) -> anyhow::Result<ServiceStatus> {
        let es = super::embedded::embedded_status();
        Ok(ServiceStatus {
            running: es.state == super::embedded::EmbeddedState::Running,
            healthy: es.last_error.is_none(),
            status_text: format!("{:?}", es.state),
            error_text: es.last_error.unwrap_or_default(),
        })
    }
}
