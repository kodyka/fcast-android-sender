package org.fcast.android.sender.capture

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.MainThread
import org.fcast.android.sender.CaptureResultBus

/**
 * Coordinates screen capture lifecycle:
 *
 *  1. Activity asks for capture → coordinator launches consent dialog.
 *  2. ScreenCaptureService delivers consent result through CaptureResultBus.
 *  3. Coordinator builds a MediaProjection and hands it to the CaptureEngine.
 *  4. Activity calls [shutdown] in onDestroy; coordinator releases everything.
 *
 * Single-instance, owned by AppGraph (step 05) or instantiated directly in
 * MainActivity.onCreate (step 04).
 */
class ScreenCaptureCoordinator(
    private val applicationContext: Context,
    private val callbacks: CaptureCallbacks,
    private val engineFactory: () -> CaptureEngine = { CaptureEngine() },
) {

    interface CaptureCallbacks {
        @MainThread fun onPermissionRequested(intent: Intent)
        @MainThread fun onCaptureStarted(width: Int, height: Int)
        @MainThread fun onCaptureStopped()
        @MainThread fun onCaptureCancelled(reason: String)
    }

    private val projectionManager: MediaProjectionManager =
        applicationContext.getSystemService(Context.MEDIA_PROJECTION_SERVICE)
            as MediaProjectionManager

    private val mainHandler = Handler(Looper.getMainLooper())

    private var engine: CaptureEngine? = null
    private var projection: MediaProjection? = null
    private var pendingConfig: CaptureConfig? = null
    private var projectionCallback: MediaProjection.Callback? = null

    private val captureListener = CaptureResultBus.Listener { result ->
        val outcome = if (result.resultCode == Activity.RESULT_OK) {
            CapturePermissionResult.Granted(result.resultCode, result.data)
        } else {
            CapturePermissionResult.Cancelled
        }
        handlePermissionResult(outcome)
    }

    @MainThread
    fun attach() {
        CaptureResultBus.setListener(captureListener)
    }

    @MainThread
    fun startCapture(config: CaptureConfig) {
        if (engine != null) {
            Log.w(TAG, "startCapture called while a capture is already running")
            return
        }
        pendingConfig = config
        callbacks.onPermissionRequested(projectionManager.createScreenCaptureIntent())
    }

    @MainThread
    fun stopCapture() {
        engine?.shutdown()
        engine = null
        projection?.let { p ->
            projectionCallback?.let { runCatching { p.unregisterCallback(it) } }
            runCatching { p.stop() }
        }
        projection = null
        projectionCallback = null
        callbacks.onCaptureStopped()
    }

    @MainThread
    fun shutdown() {
        stopCapture()
        CaptureResultBus.setListener(null)
    }

    private fun handlePermissionResult(outcome: CapturePermissionResult) {
        when (outcome) {
            is CapturePermissionResult.Granted -> startEngine(outcome.resultCode, outcome.data)
            CapturePermissionResult.Cancelled -> {
                pendingConfig = null
                callbacks.onCaptureCancelled("user")
            }
            is CapturePermissionResult.Failed -> {
                pendingConfig = null
                callbacks.onCaptureCancelled(outcome.reason)
            }
        }
    }

    @MainThread
    private fun startEngine(resultCode: Int, data: Intent) {
        val config = pendingConfig ?: run {
            callbacks.onCaptureCancelled("internal: no pending capture config")
            return
        }
        pendingConfig = null

        val mp = projectionManager.getMediaProjection(resultCode, data)
        projection = mp

        val cb = object : MediaProjection.Callback() {
            override fun onStop() {
                Log.i(TAG, "MediaProjection callback onStop")
                mainHandler.post { stopCapture() }
            }
        }
        projectionCallback = cb
        mp.registerCallback(cb, mainHandler)

        val newEngine = engineFactory().also { engine = it }
        try {
            newEngine.start(
                projection = mp,
                config = config,
                onStarted = { w, h -> mainHandler.post { callbacks.onCaptureStarted(w, h) } },
                onFatalError = { reason ->
                    mainHandler.post {
                        stopCapture()
                        callbacks.onCaptureCancelled(reason)
                    }
                },
            )
        } catch (t: Throwable) {
            Log.e(TAG, "Engine start failed", t)
            stopCapture()
            callbacks.onCaptureCancelled(t.message ?: "engine.start failed")
        }
    }

    val isCapturing: Boolean
        @MainThread get() = engine != null

    companion object {
        private const val TAG = "ScreenCaptureCoordinator"
    }
}
