# 10 · Test plan

Tier the verification — don't skip levels. Each tier rules out a
specific class of bug.

## 10.1 Rust unit tests

Co-locate in `src/backend/gstpop/embedded.rs` under `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn pick_free_port() -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    }

    #[tokio::test]
    async fn start_then_stop_is_idempotent() {
        let port = pick_free_port();
        let a = start_embedded(port).await;
        assert!(matches!(a.state,
            EmbeddedState::Running { externally_owned: false }));
        let b = start_embedded(port).await;
        assert!(matches!(b.state,
            EmbeddedState::Running { externally_owned: false }));
        let c = stop_embedded().await;
        assert!(matches!(c.state, EmbeddedState::Stopped));
    }

    #[tokio::test]
    async fn external_listener_is_adopted_and_not_killed() {
        let port = pick_free_port();
        let listener =
            tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        let a = start_embedded(port).await;
        assert!(matches!(a.state,
            EmbeddedState::Running { externally_owned: true }));
        let b = stop_embedded().await;
        // External listener is still alive.
        assert!(matches!(b.state,
            EmbeddedState::Running { externally_owned: true }));
        drop(listener);
    }

    #[tokio::test]
    async fn stop_after_failed_start_resets_state() {
        // Pre-bind with raw TcpListener so start_server fails but
        // probe_port_open succeeds → we end up Running externally_owned.
        // To exercise the *bind-fail* path, monkey-patch by injecting
        // a port we know is held by a NON-listening socket. The
        // simplest way is to skip probe_port_open (it would adopt)
        // and instead seed STATE manually for the test:
        {
            let mut st = STATE.write();
            st.state = EmbeddedState::Error;
            st.last_error = Some("simulated".into());
        }
        let s = stop_embedded().await;
        assert!(matches!(s.state, EmbeddedState::Stopped));
        assert!(s.last_error.is_none());
    }

    #[tokio::test]
    async fn port_change_triggers_internal_stop_then_start() {
        let p1 = pick_free_port();
        let p2 = pick_free_port();
        let a = start_embedded(p1).await;
        assert!(matches!(a.state,
            EmbeddedState::Running { externally_owned: false }));
        let b = start_embedded(p2).await;
        assert!(matches!(b.state,
            EmbeddedState::Running { externally_owned: false }));
        assert_eq!(b.port, p2);
        // p1 should no longer accept connections.
        assert!(!probe_port_open(p1).await);
        let _ = stop_embedded().await;
    }
}
```

Run:

```bash
$ cargo test --package android-sender backend::gstpop::embedded
```

## 10.2 Rust integration test (Slint harness)

Reuse the existing harness in
`src/backend/lifecycle.rs::tests::test_switch_media_backend_to_gstpop_integration`
(around line 220). Extend it to assert state transitions:

```rust
#[tokio::test]
async fn apply_walks_disconnected_starting_probing_ready() {
    use crate::backend::*;
    use crate::backend::lifecycle::BackendLifecycle;

    i_slint_backend_testing::init_integration_test_with_mock_time();
    let dir = tempfile::tempdir().unwrap();
    let lifecycle = std::sync::Arc::new(BackendLifecycle::new(dir.path().to_path_buf()));
    let ui = crate::MainWindow::new().unwrap();
    lifecycle.register(&ui);

    // Inject a mock service controller (see 10.4) and call apply.
    // After apply, observe Bridge.media-backend-state goes:
    //   Disconnected → Starting → Probing → Ready
    let states = std::sync::Arc::new(std::sync::Mutex::new(Vec::<crate::MediaBackendState>::new()));
    let s = states.clone();
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(50), move || {
        if let Some(ui) = weak.upgrade() {
            s.lock().unwrap().push(ui.global::<crate::Bridge>().get_media_backend_state());
        }
    });

    ui.global::<crate::Bridge>().set_media_backend(crate::MediaBackendKind::GstPop);
    ui.global::<crate::Bridge>().invoke_apply_media_backend();
    slint::run_event_loop_with_quit_on_last_window_closed(/* deadline */);

    let observed = states.lock().unwrap().clone();
    assert!(observed.contains(&crate::MediaBackendState::Starting));
    assert!(observed.contains(&crate::MediaBackendState::Ready));
}
```

(The exact event-loop drive and timer API differs by Slint version
— the gist is: walk the state machine, record transitions, assert
the sequence is monotone.)

## 10.3 Robolectric JNI/Java unit tests

New file: `app/src/test/java/org/fcast/android/sender/GstPopServiceBridgeTest.java`.

```java
package org.fcast.android.sender;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import android.content.Context;
import android.content.Intent;
import androidx.test.core.app.ApplicationProvider;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.Shadows;
import org.robolectric.shadows.ShadowApplication;

@RunWith(RobolectricTestRunner.class)
public final class GstPopServiceBridgeTest {

    @Test
    public void start_issues_intent_with_ACTION_START_and_config() {
        Context ctx = ApplicationProvider.getApplicationContext();
        GstPopServiceBridge.start(ctx, "{\"gstpop_url\":\"ws://127.0.0.1:9000\"}");

        Intent intent = Shadows.shadowOf((android.app.Application) ctx)
            .getNextStartedService();
        assertNotNull(intent);
        assertEquals(GstPopService.ACTION_START, intent.getAction());
        assertEquals("{\"gstpop_url\":\"ws://127.0.0.1:9000\"}",
                     intent.getStringExtra(GstPopService.EXTRA_CONFIG_JSON));
    }

    @Test
    public void stop_issues_intent_with_ACTION_STOP() {
        Context ctx = ApplicationProvider.getApplicationContext();
        GstPopServiceBridge.stop(ctx);
        Intent intent = Shadows.shadowOf((android.app.Application) ctx)
            .getNextStartedService();
        assertNotNull(intent);
        assertEquals(GstPopService.ACTION_STOP, intent.getAction());
    }
}
```

No native calls in the JVM unit tier — those are exercised on-device.

Run:

```bash
$ ./gradlew :app:testDebugUnitTest
```

## 10.4 Mocking the bridge in Rust tests

Wrap step 5 in a trait so tests inject a fake without going through
JNI:

```rust
// src/backend/gstpop/service.rs

pub trait ServiceController: Send + Sync {
    fn start(&self, config: &StoredBackendConfig) -> anyhow::Result<()>;
    fn stop(&self);
}

pub struct AndroidJniController;

impl ServiceController for AndroidJniController {
    fn start(&self, config: &StoredBackendConfig) -> anyhow::Result<()> {
        #[cfg(target_os = "android")] return request_service_start(config);
        #[cfg(not(target_os = "android"))] { let _ = config; Ok(()) }
    }
    fn stop(&self) {
        #[cfg(target_os = "android")] request_service_stop();
    }
}

static CONTROLLER: parking_lot::Mutex<Option<std::sync::Arc<dyn ServiceController>>> =
    parking_lot::const_mutex(None);

pub fn install_controller(c: std::sync::Arc<dyn ServiceController>) {
    *CONTROLLER.lock() = Some(c);
}

pub fn controller() -> std::sync::Arc<dyn ServiceController> {
    CONTROLLER
        .lock()
        .clone()
        .unwrap_or_else(|| std::sync::Arc::new(AndroidJniController))
}
```

In tests:

```rust
struct MockController {
    pub starts: parking_lot::Mutex<Vec<StoredBackendConfig>>,
    pub stops: parking_lot::Mutex<u32>,
}
impl ServiceController for MockController {
    fn start(&self, c: &StoredBackendConfig) -> anyhow::Result<()> {
        self.starts.lock().push(c.clone()); Ok(())
    }
    fn stop(&self) { *self.stops.lock() += 1; }
}
```

…and `BackendLifecycle::apply` calls `service::controller().start(&config)`
instead of the free function. Tests then assert `mock.starts.lock().len() == 1`.

## 10.5 Manual device flow

In order, on a real device or emulator:

1. **Apply gst-pop** → pill goes `Starting → Probing → Ready`,
   notification appears.
2. **Rotate** → notification stays, state stays Ready.
3. **Press Home (background)** → daemon still serving. Verify with:
   ```bash
   adb shell "curl -s http://127.0.0.1:9000 -o /dev/null && echo UP"
   ```
4. **Back gesture from root** (finishes activity) → service alive,
   notification alive.
5. **Task swipe-away** → with `stopWithTask="false"`, service alive.
6. **Relaunch from launcher** → pill reflects already-running daemon
   (no `Starting` flash, per §9.3).
7. **Switch to Migration in Backend page** → notification disappears,
   daemon torn down. Verify port is free:
   ```bash
   adb shell "nc -z 127.0.0.1 9000 || echo DOWN"
   ```
8. **Switch back to gst-pop** → daemon restarts cleanly.
9. **Simulate process death**:
   ```bash
   adb shell am kill org.fcast.android.sender
   ```
   → service is restarted by `START_STICKY` with null intent → per
   step 4.1 it stops cleanly → next Apply restarts it.
10. **Hold the port** to force a bind failure:
    ```bash
    adb shell "nc -l -p 9000 &"
    ```
    → Apply gst-pop → expect Error pill with bind-failure text.
    Notification disappears (step 4.1 `isErrorState`).
    Then `adb shell pkill nc` and Apply again → Ready.

## 10.6 Log assertions

Grep `adb logcat` for exactly one of these per process lifetime:

- `Embedded gst-pop running on 127.0.0.1:9000`
- `External gst-pop already on 127.0.0.1:9000; adopting`

More than one of either means the lifecycle is double-starting.

You should see exactly one `stop_embedded: dropping handle` per
explicit teardown.

## 10.7 CI

The existing `gstpop-smoke` workflow already exercises the
externally-owned path (dockerised daemon on 9000). With probe()
tightened in step 6, it just connects — no implicit start, no port
collision. The `build-android-arm64-debug` workflow builds the cdylib
including the new JNI symbols; verify the new symbols are present:

```yaml
# .github/workflows/android.yml (extend the existing build job)
- name: Verify JNI symbols
  run: |
    nm --defined-only app/src/main/jniLibs/arm64-v8a/libfcastsender.so \
      | grep -c GstPopServiceBridge
    # Expect exactly 3 (Start, Stop, GetStatus).
```

Next: [11-cleanup-checklist.md](./11-cleanup-checklist.md).
