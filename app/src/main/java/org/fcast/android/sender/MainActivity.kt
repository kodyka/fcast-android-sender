package org.fcast.android.sender

import android.app.NativeActivity
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.hardware.display.DisplayManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.util.Log
import android.view.KeyEvent
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import org.fcast.android.sender.capture.CaptureConfig
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.discovery.Discoverer
import org.fcast.android.sender.qr.QrScannerLauncher
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.shell.SenderController
import org.fcast.android.sender.shell.UiState
import org.freedesktop.gstreamer.GStreamer
import java.nio.ByteBuffer

class MainActivity : NativeActivity(), DisplayManager.DisplayListener {

    private lateinit var displayManager: DisplayManager
    private lateinit var coordinator: ScreenCaptureCoordinator
    private lateinit var qr: QrScannerLauncher
    private lateinit var controller: SenderController

    private val activityScope = MainScope()

    private val captureCallbacks = object : ScreenCaptureCoordinator.CaptureCallbacks {
        @Suppress("DEPRECATION")
        override fun onPermissionRequested(intent: Intent) {
            startActivityForResult(intent, REQ_PROJECTION)
        }
        override fun onCaptureStarted(width: Int, height: Int) {
            // TODO(step-10): once Slint→Kotlin wires backend start through the controller,
            // uiState will be Connected here and the MIGRATION fallback will stop firing.
            val kind = (controller.uiState.value as? UiState.Connected)?.kind ?: BackendKind.MIGRATION
            controller.onCaptureStartedFromCoordinator(kind, width, height)
            nativeCaptureStarted()
        }
        override fun onCaptureStopped() {
            controller.onCaptureStoppedFromCoordinator()
            nativeCaptureStopped()
        }
        override fun onCaptureCancelled(reason: String) {
            controller.onCaptureStoppedFromCoordinator()
            nativeCaptureCancelled()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        try {
            GStreamer.init(this)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to init GStreamer $e")
            finish()
        }

        Discoverer(this)

        coordinator = (application as FcastApp).graph.newCaptureCoordinator(captureCallbacks)
        coordinator.attach()

        qr = org.fcast.android.sender.qr.RealQrScannerLauncher(this)
        controller = SenderController((application as FcastApp).graph.runtime, coordinator, qr)

        activityScope.launch {
            controller.uiState.collect { onUiStateChanged(it) }
        }

        displayManager = getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
        displayManager.registerDisplayListener(this, Handler(mainLooper))

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 101)
            }
        }
    }

    override fun onDestroy() {
        controller.shutdown()
        coordinator.shutdown()
        activityScope.cancel()
        runCatching { displayManager.unregisterDisplayListener(this) }
        super.onDestroy()
    }

    override fun onStop() {
        // onStop intentionally left out — capture must survive background.
        // See refactor step 03.4 for context.
        super.onStop()
    }

    override fun onBackPressed() {
        Log.d(TAG, "onBackPressed")
        nativeBackPressed()
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (event.keyCode == KeyEvent.KEYCODE_BACK && event.action == KeyEvent.ACTION_UP) {
            Log.d(TAG, "dispatchKeyEvent ACTION_UP KEYCODE_BACK")
            nativeBackPressed()
            return true
        }
        return super.dispatchKeyEvent(event)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            Log.d(TAG, "onKeyDown KEYCODE_BACK")
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            Log.d(TAG, "onKeyUp KEYCODE_BACK")
            nativeBackPressed()
            return true
        }
        return super.onKeyUp(keyCode, event)
    }

    override fun onDisplayAdded(displayId: Int) {}
    override fun onDisplayRemoved(displayId: Int) {}
    override fun onDisplayChanged(displayId: Int) {}

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
    }

    @Deprecated("Use ActivityResultLauncher; see QrScannerLauncher.")
    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        when (requestCode) {
            REQ_PROJECTION -> {
                if (resultCode == RESULT_OK) {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                        val serviceIntent = Intent(this, ScreenCaptureService::class.java).apply {
                            action = ScreenCaptureService.ACTION_RESULT
                            putExtra("resultCode", resultCode)
                            putExtra("data", data)
                        }
                        Log.d(TAG, "Starting foreground service SDK=${Build.VERSION.SDK_INT}")
                        runCatching { startForegroundService(serviceIntent) }
                            .onFailure { Log.e(TAG, "Failed to start foreground service: $it") }
                    } else {
                        Log.d(TAG, "Starting capture")
                        CaptureResultBus.deliver(resultCode, data ?: Intent())
                    }
                } else if (resultCode == RESULT_CANCELED) {
                    Log.d(TAG, "Media projection Canceled")
                    CaptureResultBus.deliver(resultCode, Intent())
                }
            }
            QrScannerLauncher.REQUEST_CODE -> {
                if (resultCode == RESULT_OK) {
                    val result = data?.getStringExtra("SCAN_RESULT")
                    nativeQrScanResult(result ?: "")
                }
            }
        }
    }

    // Called from native code
    private fun startScreenCapture(scaleWidth: Int, scaleHeight: Int, maxFramerate: Int) {
        Log.d(TAG, "Requesting screen capture permissions: scaleWidth=$scaleWidth scaleHeight=$scaleHeight maxFramerate=$maxFramerate")
        coordinator.startCapture(CaptureConfig(scaleWidth, scaleHeight, maxFramerate))
    }

    // Called from native code
    private fun stopCapture() {
        coordinator.stopCapture()
    }

    // Called from native code
    private fun scanQr() {
        controller.scanQr()
    }

    // Called from native code
    private fun finishApp() {
        runOnUiThread { finish() }
    }

    private fun onUiStateChanged(state: UiState) {
        // AppState ordinals: 0=Disconnected, 1=Connecting, 3=WaitingForMedia, 4=Casting.
        // BannerSeverity ordinals: 0=info, 1=success, 2=warning, 3=error.
        // Error maps to Disconnected + non-empty error banner so Slint can surface it.
        // TODO(step-12): banner writes here can race with Application::flash_banner's
        // auto-hide timer. Coordinate via the generation counter in Application.
        val (slintState, banner, severity) = when (state) {
            is UiState.Disconnected -> Triple(0, "", 0)
            is UiState.Starting     -> Triple(1, "", 0)
            is UiState.Connected    -> Triple(3, state.message ?: "", 0)
            is UiState.Casting      -> Triple(4, "", 0)
            is UiState.Error        -> Triple(0, state.message, 3)
        }
        nativeSlintApplyState(slintState, banner, severity)
    }

    // ── JNI symbol shims ─────────────────────────────────────────────────
    // Names match Rust's Java_org_fcast_android_sender_MainActivity_native*.
    external fun nativeBackPressed()
    external fun nativeCaptureStarted()
    external fun nativeCaptureStopped()
    external fun nativeCaptureCancelled()
    external fun nativeQrScanResult(result: String)
    external fun nativeSlintApplyState(state: Int, banner: String, severity: Int)

    companion object {
        init {
            System.loadLibrary("gstreamer_android")
            System.loadLibrary("fcastsender")
        }

        private const val TAG = "MainActivity"
        const val REQ_PROJECTION = 1
        const val ACTION_MEDIA_PROJECTION_STARTED = "org.fcast.android.sender.MEDIA_PROJECTION_STARTED"

        @JvmStatic
        external fun nativeProcessFrame(width: Int, height: Int, bufferY: ByteBuffer, bufferU: ByteBuffer, bufferV: ByteBuffer)

        @JvmStatic
        external fun nativeGraphCommand(payloadJson: String): String
    }

}
