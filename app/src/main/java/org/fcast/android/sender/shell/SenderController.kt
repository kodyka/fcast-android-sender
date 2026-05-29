package org.fcast.android.sender.shell

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.fcast.android.sender.capture.CaptureConfig
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.qr.QrScannerLauncher
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.runtime.BackendStatus
import org.fcast.android.sender.runtime.RuntimeBridge

/**
 * State-holder + coordinator for the Android shell.
 *
 * Threading: the controller schedules work on its own SupervisorJob-based
 * scope. All public methods are non-suspend and return a Job; callers
 * usually fire-and-forget. Tests use kotlinx.coroutines.test.TestScope.
 */
class SenderController(
    private val runtime: RuntimeBridge,
    private val coordinator: ScreenCaptureCoordinator,
    private val qr: QrScannerLauncher,
    private val dispatcher: CoroutineDispatcher = Dispatchers.Default,
) {
    private val scope = CoroutineScope(SupervisorJob() + dispatcher)

    private val _uiState = MutableStateFlow<UiState>(UiState.Disconnected)
    val uiState: StateFlow<UiState> = _uiState.asStateFlow()

    fun startBackend(kind: BackendKind, configJson: String): Job = scope.launch {
        _uiState.value = UiState.Starting(kind)
        val status: BackendStatus = runtime.startEmbeddedBackend(kind, configJson)
        _uiState.value = when {
            status.isError   -> UiState.Error(status.message ?: "unknown")
            status.isRunning -> UiState.Connected(kind, status.message)
            else             -> UiState.Disconnected
        }
    }

    fun stopBackend(kind: BackendKind): Job = scope.launch {
        runtime.stopEmbeddedBackend(kind)
        _uiState.value = UiState.Disconnected
    }

    fun refreshStatus(kind: BackendKind): Job = scope.launch {
        val status = runtime.backendStatus(kind)
        when {
            status.isError   -> _uiState.value = UiState.Error(status.message ?: "unknown")
            status.isRunning -> _uiState.value = UiState.Connected(kind, status.message)
            else             -> _uiState.value = UiState.Disconnected
        }
    }

    fun startCasting(kind: BackendKind, config: CaptureConfig) {
        coordinator.startCapture(config)
        _uiState.value = UiState.Starting(kind)
    }

    fun stopCasting() {
        coordinator.stopCapture()
        _uiState.value = UiState.Disconnected
    }

    fun scanQr() {
        qr.launch()
    }

    /** Called by ScreenCaptureCoordinator.CaptureCallbacks.onCaptureStarted. */
    fun onCaptureStartedFromCoordinator(kind: BackendKind, w: Int, h: Int) {
        _uiState.value = UiState.Casting(kind, w, h)
    }

    /** Called by ScreenCaptureCoordinator.CaptureCallbacks.onCaptureStopped. */
    fun onCaptureStoppedFromCoordinator() {
        _uiState.value = UiState.Disconnected
    }

    fun shutdown() {
        scope.cancel()
    }
}
