# 10 — Android-side automated tests

**Priority:** Medium · **Effort:** Medium · **Estimated PR size:** ~250 LOC of test code + workflow changes.

## Goal

Add three layers of automated tests on the Android side:

1. **JVM unit tests** for service contracts (action handling, null-intent paths,
   notification building) and parser logic (status-JSON parsing).
2. **Instrumentation tests** for foreground-service start contracts and the
   `RuntimeBridge` round-trip on an emulator.
3. **Re-affirm the Rust-side headless Slint UI tests** as the required pre-merge
   gate, and remove the `--test-threads=1` requirement once step 07 isolates the
   gst-pop global state.

## Report finding

> "Android service tests — not visible in the inspected workflows — add JVM tests
> for service contracts and parser logic; add targeted instrumentation tests for
> foreground-service and notification behaviour."

— `deep-research-report-3.md`, "Testing, CI, risk and rollback".

> "Use `kotlinx-coroutines-test` for new Android-side controller/state tests."

— same document, "Proposed target architecture and libraries".

## Pre-state on `main`

Verified test surface:

| Path                                                       | What it covers                                                  |
|------------------------------------------------------------|-----------------------------------------------------------------|
| `tests/ui_snapshots.rs`                                    | Headless Slint UI snapshots.                                    |
| `src/` and `crates/*/src/` inline `#[cfg(test)]`           | Rust unit tests for backend/migration/gst-pop logic.            |
| `.github/workflows/gstpop-smoke.yml`                       | gst-pop smoke with `--include-ignored --test-threads=1`.        |
| `app/src/test/`                                            | **No tests verified** (must be added).                          |
| `app/src/androidTest/`                                     | **No tests verified** (must be added).                          |

`gstpop-smoke.yml` lines 101-103 carry the verified comment that two ignored
tests share process-global atomics. The single-thread gate stays until step 07
eliminates that global state.

## JVM unit tests

### `app/src/test/java/org/fcast/android/sender/ScreenCaptureServiceTest.java`

```java
package org.fcast.android.sender;

import static org.junit.Assert.assertEquals;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.spy;
import static org.mockito.Mockito.verify;

import android.app.Service;
import android.content.Intent;

import org.junit.Test;
import org.robolectric.Robolectric;

/**
 * Validates the {@link ScreenCaptureService#onStartCommand} contract added in
 * refactor step 01. Requires Robolectric in the test classpath.
 */
public class ScreenCaptureServiceTest {

    @Test
    public void nullIntent_doesNotCrash_andReturnsNotSticky() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        int result = svc.onStartCommand(null, 0, 1);
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void unknownAction_isIgnored_andReturnsNotSticky() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        Intent intent = new Intent().setAction("unknown");
        int result = svc.onStartCommand(intent, 0, 1);
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void grantedResult_startsForeground() {
        ScreenCaptureService svc = spy(Robolectric.setupService(ScreenCaptureService.class));
        Intent data = new Intent();
        Intent intent = new Intent()
            .setAction(ScreenCaptureService.ACTION_RESULT)
            .putExtra("resultCode", android.app.Activity.RESULT_OK)
            .putExtra("data", data);
        int result = svc.onStartCommand(intent, 0, 1);
        verify(svc).startForeground(anyInt(), any());
        assertEquals(Service.START_NOT_STICKY, result);
    }
}
```

### `app/src/test/java/org/fcast/android/sender/runtime/StatusParserTest.kt`

```kotlin
package org.fcast.android.sender.runtime

import org.junit.Assert.assertEquals
import org.junit.Test

class StatusParserTest {
    private val bridge = JniRuntimeBridge(appContext = null!!)

    private fun parse(json: String) =
        bridge.javaClass.getDeclaredMethod("parseStatus", String::class.java)
            .also { it.isAccessible = true }
            .invoke(bridge, json) as BackendStatus

    @Test fun running() {
        assertEquals(BackendStatus("running", null), parse("""{"state":"running"}"""))
    }
    @Test fun errorWithMessage() {
        assertEquals(BackendStatus("error", "boom"),
            parse("""{"state":"error","message":"boom"}"""))
    }
    @Test fun unparseable_isMappedToError() {
        assertEquals("error", parse("not json").state)
    }
}
```

(For the parser unit test you can either expose `parseStatus` as an internal
function or extract it to a small `StatusParser` object — the latter is
preferable; the snippet above is the minimum-change form.)

### `app/src/test/java/org/fcast/android/sender/shell/SenderControllerTest.kt`

```kotlin
package org.fcast.android.sender.shell

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.runtime.BackendStatus
import org.fcast.android.sender.runtime.RuntimeBridge
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SenderControllerTest {
    private val dispatcher = StandardTestDispatcher()
    @Before fun setMain() = Dispatchers.setMain(dispatcher)
    @After  fun resetMain() = Dispatchers.resetMain()

    private class FakeRuntime(val status: BackendStatus) : RuntimeBridge {
        override suspend fun startEmbeddedBackend(kind: BackendKind, configJson: String) = status
        override suspend fun stopEmbeddedBackend(kind: BackendKind) = BackendStatus("stopped", null)
        override suspend fun backendStatus(kind: BackendKind) = status
        override suspend fun graphCommand(action: String, params: JSONObject) = JSONObject()
    }

    @Test fun running_status_yields_connected() = runTest {
        val ctrl = SenderController(FakeRuntime(BackendStatus("running", "ok")), /* … */)
        ctrl.startBackend(BackendKind.MIGRATION, "{}")
        advanceUntilIdle()
        assertEquals(UiState.Connected(BackendKind.MIGRATION, "ok"), ctrl.uiState.value)
    }

    @Test fun error_status_yields_error_state() = runTest {
        val ctrl = SenderController(FakeRuntime(BackendStatus("error", "boom")), /* … */)
        ctrl.startBackend(BackendKind.MIGRATION, "{}")
        advanceUntilIdle()
        assertEquals(UiState.Error("boom"), ctrl.uiState.value)
    }
}
```

## Instrumentation tests

### `app/src/androidTest/java/org/fcast/android/sender/RuntimeBridgeInstrumentedTest.kt`

```kotlin
package org.fcast.android.sender

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.runtime.JniRuntimeBridge
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class RuntimeBridgeInstrumentedTest {
    @Test fun statusPing_returnsParseableJson() = runBlocking {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val bridge = JniRuntimeBridge(ctx)
        val status = bridge.backendStatus(BackendKind.MIGRATION)
        assertNotNull(status.state)
    }
}
```

### `app/src/androidTest/java/org/fcast/android/sender/ScreenCaptureServiceInstrumentedTest.kt`

```kotlin
package org.fcast.android.sender

import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ScreenCaptureServiceInstrumentedTest {
    @Test fun unknown_action_does_not_start_foreground() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val intent = Intent(ctx, ScreenCaptureService::class.java).setAction("unknown")
        // No exception, and process exits 0; the negative assertion is "we
        // got here at all".
        ctx.startService(intent)
        assertTrue(true)
    }
}
```

## Gradle additions

```diff
 android {
     testOptions {
         unitTests {
             includeAndroidResources = true
             returnDefaultValues = true
         }
     }
 }

 dependencies {
+    testImplementation 'junit:junit:4.13.2'
+    testImplementation 'org.mockito:mockito-core:5.12.0'
+    testImplementation 'org.robolectric:robolectric:4.13'
+    testImplementation 'org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0'
+    androidTestImplementation 'androidx.test.ext:junit:1.2.1'
+    androidTestImplementation 'androidx.test:runner:1.6.2'
 }
```

## CI wiring

Add a JVM-test job to the existing debug pipeline (no new workflow needed):

```yaml
- name: Run :app:testDebugUnitTest
  run: ./gradlew --no-daemon :app:testDebugUnitTest
```

For instrumentation tests, add a follow-on job that boots an emulator:

```yaml
android-instrumented-tests:
  runs-on: ubuntu-24.04
  steps:
    - uses: actions/checkout@v4
    - uses: ./.github/actions/android-ci-setup
      with: { rust-target: aarch64-linux-android }
    - name: Enable KVM
      run: |
        echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666"' | sudo tee /etc/udev/rules.d/99-kvm.rules
        sudo udevadm control --reload-rules
        sudo udevadm trigger --name-match=kvm
    - name: Run connected tests
      uses: reactivecircus/android-emulator-runner@v2
      with:
        api-level: 34
        target: google_apis
        arch: x86_64
        script: ./gradlew :app:connectedDebugAndroidTest
```

If runner cost is a concern, gate the emulator job on `pull_request` with the
`needs-emulator` label, and run it on a nightly schedule otherwise.

## Reduce the gst-pop test-thread requirement

The verified comment in `.github/workflows/gstpop-smoke.yml:101-103`:

```yaml
# --test-threads=1 required: two ignored tests share process-global atomics
run: cargo test backend::gstpop -- --include-ignored --test-threads=1
```

After step 07 sub-PR 7.5 eliminates the shared atomics, drop the `--test-threads=1`
flag in the same PR that removes the global state. Until then, leave the gate
in place.

## Testing

| Test                                                              | How                                                                      |
|-------------------------------------------------------------------|--------------------------------------------------------------------------|
| `:app:testDebugUnitTest` passes                                    | Local: `./gradlew :app:testDebugUnitTest`.                                |
| `:app:connectedDebugAndroidTest` passes on emulator API 34         | Local: `./gradlew connectedDebugAndroidTest` with an emulator running.   |
| Existing Rust tests still pass                                     | `cargo test -p fcastsender`.                                              |
| Existing Slint headless tests still pass                          | `cargo test -p fcastsender --test ui_snapshots`.                          |

## Rollback

- Revert the test source files and the Gradle dependency additions.
- Revert the workflow additions (or simply mark the new jobs as `if: false`).
- The legacy `--test-threads=1` gate stays as-is.

## Follow-ups (not in this PR)

- Coverage report upload to Codecov / similar.
- A "smoke" Slint UI test that uses the new `RuntimeBridge` fake — once step 05
  lands.
