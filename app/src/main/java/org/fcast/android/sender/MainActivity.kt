package org.fcast.android.sender

import android.app.NativeActivity
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.hardware.display.DisplayManager
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.util.Log
import android.view.KeyEvent
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch
import org.fcast.android.sender.capture.CaptureConfig
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.qr.QrScannerLauncher
import org.fcast.android.sender.shell.SenderController
import org.fcast.android.sender.shell.UiState
import org.freedesktop.gstreamer.GStreamer
import org.json.JSONException
import org.json.JSONObject
import java.net.Inet6Address
import java.net.InetAddress
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.stream.Collectors

internal class FCastDiscoveryListener(private val nsdManager: NsdManager) : NsdManager.DiscoveryListener {

    override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
        Log.e(TAG, "Failed to start discovery errorCode=$errorCode")
    }

    override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
        Log.e(TAG, "Failed to stop discovery errorCode=$errorCode")
    }

    override fun onDiscoveryStarted(serviceType: String) {
        Log.i(TAG, "Discovery started")
    }

    override fun onDiscoveryStopped(serviceType: String) {
        Log.i(TAG, "Discovery stopped")
    }

    override fun onServiceFound(serviceInfo: NsdServiceInfo) {
        Log.i(TAG, "Service found serviceInfo=$serviceInfo")

        var addrs: List<InetAddress> = emptyList()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            addrs = serviceInfo.hostAddresses
        } else {
            val hostAddr = serviceInfo.host
            if (hostAddr != null) addrs = listOf(hostAddr)
        }
        val addrsB = addrs.stream().map { addrConvert(it) }.collect(Collectors.toList())
        serviceFound(serviceInfo.serviceName, addrsB, serviceInfo.port)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            nsdManager.registerServiceInfoCallback(serviceInfo, Runnable::run, object : NsdManager.ServiceInfoCallback {
                override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {}
                override fun onServiceUpdated(updated: NsdServiceInfo) {
                    serviceFound(
                        updated.serviceName,
                        updated.hostAddresses.stream().map { addrConvert(it) }.collect(Collectors.toList()),
                        updated.port,
                    )
                }
                override fun onServiceLost() { serviceLost(serviceInfo.serviceName) }
                override fun onServiceInfoCallbackUnregistered() {}
            })
        } else {
            nsdManager.resolveService(serviceInfo, object : NsdManager.ResolveListener {
                override fun onResolveFailed(si: NsdServiceInfo, errorCode: Int) {
                    Log.e(TAG, "Service failed to resolve serviceInfo=$si")
                }
                override fun onServiceResolved(si: NsdServiceInfo) {
                    Log.i(TAG, "Service resolved serviceInfo=$si")
                    val addr = si.host
                    if (addr != null) {
                        serviceFound(si.serviceName, listOf(addrConvert(addr)), si.port)
                    }
                }
            })
        }
    }

    override fun onServiceLost(serviceInfo: NsdServiceInfo) {
        Log.i(TAG, "Service lost serviceInfo=$serviceInfo")
        serviceLost(serviceInfo.serviceName)
    }

    private external fun serviceFound(name: String, addrs: List<ByteBuffer>, port: Int)
    private external fun serviceLost(name: String)

    companion object {
        private const val TAG = "FCastDiscoveryListener"

        private fun addrConvert(addr: InetAddress): ByteBuffer {
            val addrB = addr.address
            val buffer = ByteBuffer.allocateDirect(addrB.size)
            buffer.put(addrB)
            if (addr is Inet6Address) {
                buffer.order(ByteOrder.LITTLE_ENDIAN).putInt(addr.scopeId)
            }
            return buffer
        }
    }
}

internal class Discoverer(context: Context) {
    init {
        val nsdManager = context.getSystemService(Context.NSD_SERVICE) as NsdManager
        nsdManager.discoverServices("_fcast._tcp", NsdManager.PROTOCOL_DNS_SD, FCastDiscoveryListener(nsdManager))
    }
}

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
        override fun onCaptureStarted(width: Int, height: Int) { nativeCaptureStarted() }
        override fun onCaptureStopped() { nativeCaptureStopped() }
        override fun onCaptureCancelled(reason: String) { nativeCaptureCancelled() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        try {
            GStreamer.init(this)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to init GStreamer $e")
            finish()
        }

        @Suppress("UNUSED_VARIABLE")
        val discoverer = Discoverer(this)

        coordinator = (application as FcastApp).graph.newCaptureCoordinator(captureCallbacks)
        coordinator.attach()

        qr = QrScannerLauncher(this)
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
        qr.launch()
    }

    // Called from native code
    private fun finishApp() {
        runOnUiThread { finish() }
    }

    private fun onUiStateChanged(state: UiState) {
        // Slint observes its own globals via the Rust shim (see step 08.8).
        // This hook is reserved for non-Slint affordances (analytics, etc.).
    }

    fun graphCommand(payloadJson: String?): GraphCommandResponse {
        val responseJson = nativeGraphCommand(payloadJson ?: "")
        return parseGraphCommandResponse(responseJson)
    }

    fun graphCommand(action: String, params: JSONObject?): GraphCommandResponse {
        return try {
            val payload = JSONObject().put(action, params ?: JSONObject())
            graphCommand(payload)
        } catch (e: JSONException) {
            GraphCommandResponse.error("Invalid graph command payload: ${e.message}", null)
        }
    }

    fun graphCommand(payload: JSONObject?): GraphCommandResponse {
        return graphCommand(payload?.toString() ?: "")
    }

    private fun parseGraphCommandResponse(responseJson: String?): GraphCommandResponse {
        if (responseJson.isNullOrEmpty()) {
            return GraphCommandResponse.error("Empty graph command response", null)
        }
        return try {
            val root = JSONObject(responseJson)
            when (val result = root.opt("result")) {
                is String -> {
                    if (result == "success") GraphCommandResponse.success(null, root)
                    else GraphCommandResponse.error("Unexpected graph result string: $result", root)
                }
                is JSONObject -> {
                    when {
                        result.has("error") ->
                            GraphCommandResponse.error(result.optString("error", "Unknown graph command error"), root)
                        result.has("info") ->
                            GraphCommandResponse.success(result.optJSONObject("info"), root)
                        else ->
                            GraphCommandResponse.error("Unsupported graph result shape", root)
                    }
                }
                else -> GraphCommandResponse.error("Unsupported graph result shape", root)
            }
        } catch (e: JSONException) {
            Log.e(TAG, "Failed to parse graph command response: $responseJson", e)
            GraphCommandResponse.error("Invalid graph response JSON: ${e.message}", null)
        }
    }

    // ── JNI symbol shims ─────────────────────────────────────────────────
    // Names match Rust's Java_org_fcast_android_sender_MainActivity_native*.
    external fun nativeBackPressed()
    external fun nativeCaptureStarted()
    external fun nativeCaptureStopped()
    external fun nativeCaptureCancelled()
    external fun nativeQrScanResult(result: String)

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

    data class GraphCommandResponse(
        val success: Boolean,
        val error: String?,
        val info: JSONObject?,
        val raw: JSONObject?,
    ) {
        companion object {
            @JvmStatic
            fun success(info: JSONObject?, raw: JSONObject?) = GraphCommandResponse(true, null, info, raw)
            @JvmStatic
            fun error(error: String?, raw: JSONObject?) = GraphCommandResponse(false, error, null, raw)
        }
    }
}
