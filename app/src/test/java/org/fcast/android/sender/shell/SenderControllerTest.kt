package org.fcast.android.sender.shell

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.fcast.android.sender.capture.CaptureConfig
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.qr.QrScannerLauncher
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

    @Before fun setUp() { Dispatchers.setMain(dispatcher) }
    @After  fun tearDown() { Dispatchers.resetMain() }

    private class FakeRuntime(
        val startStatus: BackendStatus = BackendStatus("running", null, null),
        val statusStatus: BackendStatus = BackendStatus("running", null, null),
    ) : RuntimeBridge {
        var lastStartKind: BackendKind? = null
        var lastStartConfig: String? = null
        var stopCalledWith: BackendKind? = null
        override suspend fun startEmbeddedBackend(kind: BackendKind, configJson: String): BackendStatus {
            lastStartKind = kind; lastStartConfig = configJson; return startStatus
        }
        override suspend fun stopEmbeddedBackend(kind: BackendKind): BackendStatus {
            stopCalledWith = kind; return BackendStatus("stopped", null, null)
        }
        override suspend fun backendStatus(kind: BackendKind) = statusStatus
        override suspend fun graphCommand(action: String, params: JSONObject) = JSONObject()
    }

    private object NoOpCoordinator : ScreenCaptureCoordinator {
        override fun attach() {}
        override fun startCapture(config: CaptureConfig) {}
        override fun stopCapture() {}
        override fun shutdown() {}
        override val isCapturing: Boolean get() = false
    }

    private object NoOpQrLauncher : QrScannerLauncher {
        override fun launch() {}
    }

    private fun controller(runtime: RuntimeBridge) =
        SenderController(runtime, NoOpCoordinator, NoOpQrLauncher)

    @Test
    fun startBackend_running_yieldsConnected() = runTest {
        val ctrl = controller(FakeRuntime(BackendStatus("running", "ok", null)))
        ctrl.startBackend(BackendKind.MIGRATION, "{}").join()
        advanceUntilIdle()
        assertEquals(UiState.Connected(BackendKind.MIGRATION, "ok"), ctrl.uiState.value)
    }

    @Test
    fun startBackend_error_yieldsError() = runTest {
        val ctrl = controller(FakeRuntime(BackendStatus("error", "boom", null)))
        ctrl.startBackend(BackendKind.MIGRATION, "{}").join()
        advanceUntilIdle()
        assertEquals(UiState.Error("boom"), ctrl.uiState.value)
    }

    @Test
    fun startBackend_unknown_yieldsDisconnected() = runTest {
        val ctrl = controller(FakeRuntime(BackendStatus("queued", null, null)))
        ctrl.startBackend(BackendKind.GSTPOP, "{}").join()
        advanceUntilIdle()
        assertEquals(UiState.Disconnected, ctrl.uiState.value)
    }

    @Test
    fun stopBackend_yieldsDisconnected() = runTest {
        val ctrl = controller(FakeRuntime(BackendStatus("running", null, null)))
        ctrl.startBackend(BackendKind.GSTPOP, "{}").join()
        ctrl.stopBackend(BackendKind.GSTPOP).join()
        advanceUntilIdle()
        assertEquals(UiState.Disconnected, ctrl.uiState.value)
    }

    @Test
    fun coordinatorCallback_yieldsCasting() = runTest {
        val ctrl = controller(FakeRuntime())
        ctrl.onCaptureStartedFromCoordinator(BackendKind.MIGRATION, 1280, 720)
        assertEquals(UiState.Casting(BackendKind.MIGRATION, 1280, 720), ctrl.uiState.value)
    }
}
