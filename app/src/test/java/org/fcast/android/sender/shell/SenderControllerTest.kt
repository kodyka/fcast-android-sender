package org.fcast.android.sender.shell

import kotlinx.coroutines.test.runTest
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.runtime.BackendStatus
import org.fcast.android.sender.runtime.RuntimeBridge
import org.json.JSONObject
import org.junit.Ignore
import org.junit.Test

class SenderControllerTest {

    // Fakes are referenced inside @Ignore'd bodies and will be replaced by
    // proper doubles in step 10 (Robolectric setup).
    @Suppress("unused")
    private class FakeRuntime(
        private val startStatus: BackendStatus = BackendStatus("running", null, null),
    ) : RuntimeBridge {
        override suspend fun startEmbeddedBackend(kind: BackendKind, configJson: String) = startStatus
        override suspend fun stopEmbeddedBackend(kind: BackendKind) = BackendStatus("stopped", null, null)
        override suspend fun backendStatus(kind: BackendKind) = startStatus
        override suspend fun graphCommand(action: String, params: JSONObject) = JSONObject()
    }

    @Suppress("unused")
    private class FakeRuntimeError : RuntimeBridge {
        override suspend fun startEmbeddedBackend(kind: BackendKind, configJson: String) =
            BackendStatus("error", "daemon not responding", null)
        override suspend fun stopEmbeddedBackend(kind: BackendKind) = BackendStatus("stopped", null, null)
        override suspend fun backendStatus(kind: BackendKind) = BackendStatus("error", "daemon not responding", null)
        override suspend fun graphCommand(action: String, params: JSONObject) = JSONObject()
    }

    // SenderController requires ScreenCaptureCoordinator (Android context)
    // and QrScannerLauncher (Activity). Both fakes land in step 10 (Robolectric).

    @Ignore("Requires Robolectric context — wired in step 10")
    @Test
    fun startBackend_transitionsToConnected() = runTest {
        // step 10: val ctrl = SenderController(FakeRuntime(), FakeCoordinator(), FakeQrLauncher())
        // ctrl.startBackend(BackendKind.GSTPOP, "{}").join()
        // assertEquals(UiState.Connected(BackendKind.GSTPOP, null), ctrl.uiState.value)
    }

    @Ignore("Requires Robolectric context — wired in step 10")
    @Test
    fun startBackend_transitionsToError() = runTest {
        // step 10: val ctrl = SenderController(FakeRuntimeError(), FakeCoordinator(), FakeQrLauncher())
        // ctrl.startBackend(BackendKind.GSTPOP, "{}").join()
        // assertTrue(ctrl.uiState.value is UiState.Error)
    }
}
