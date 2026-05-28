package org.fcast.android.sender;

import static android.opengl.EGLExt.EGL_OPENGL_ES3_BIT_KHR;
import static android.opengl.GLES11Ext.GL_TEXTURE_EXTERNAL_OES;
import static android.opengl.GLES20.*;
import static android.opengl.GLES30.*;

import android.app.Activity;
import android.app.NativeActivity;
import android.content.Context;
import android.content.Intent;
import android.content.res.*;
import android.graphics.SurfaceTexture;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.media.projection.MediaProjection;
import android.media.projection.MediaProjectionManager;
import android.net.nsd.NsdManager;
import android.net.nsd.NsdServiceInfo;
import android.opengl.EGL14;
import android.opengl.EGLConfig;
import android.opengl.EGLContext;
import android.opengl.EGLDisplay;
import android.opengl.EGLSurface;
import android.os.*;
import android.util.DisplayMetrics;
import android.util.Log;
import android.view.*;

import androidx.annotation.MainThread;
import androidx.annotation.NonNull;

import org.fcast.android.sender.capture.CaptureConfig;
import org.fcast.android.sender.capture.CaptureEngine;
import org.fcast.android.sender.capture.ScreenCaptureCoordinator;
import org.fcast.android.sender.qr.QrScannerLauncher;

import org.freedesktop.gstreamer.GStreamer;
import org.json.JSONException;
import org.json.JSONObject;

import java.net.Inet6Address;
import java.net.InetAddress;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.FloatBuffer;
import java.time.Duration;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.atomic.*;
import java.util.concurrent.locks.*;
import java.util.stream.Collectors;

class FCastDiscoveryListener implements NsdManager.DiscoveryListener {
    private static final String TAG = "FCastDiscoveryListener";
    private final NsdManager nsdManager;


    FCastDiscoveryListener(NsdManager nsdManager) {
        this.nsdManager = nsdManager;
    }

    private static ByteBuffer addrConvert(InetAddress addr) {
        byte[] addrB = addr.getAddress();
        ByteBuffer buffer = ByteBuffer.allocateDirect(addrB.length);
        buffer.put(addrB);

        if (addr.getClass() == Inet6Address.class) {
            int scopeId = ((Inet6Address) addr).getScopeId();
            buffer.order(ByteOrder.LITTLE_ENDIAN).putInt(scopeId);
        }

        return buffer;
    }

    @Override
    public void onStartDiscoveryFailed(String serviceType, int errorCode) {
        Log.e(TAG, "Failed to start discovery errorCode=" + errorCode);
    }

    @Override
    public void onStopDiscoveryFailed(String serviceType, int errorCode) {
        Log.e(TAG, "Failed to stop discovery errorCode=" + errorCode);
    }

    @Override
    public void onDiscoveryStarted(String serviceType) {
        Log.i(TAG, "Discovery started");
    }

    @Override
    public void onDiscoveryStopped(String serviceType) {
        Log.i(TAG, "Discovery stopped");
    }

    @Override
    public void onServiceFound(NsdServiceInfo serviceInfo) {
        Log.i(TAG, "Service found serviceInfo=" + serviceInfo);

        List<InetAddress> addrs = List.of();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            addrs = serviceInfo.getHostAddresses();
        } else {
            InetAddress hostAddr = serviceInfo.getHost();
            if (hostAddr != null) {
                addrs = List.of(hostAddr);
            }
        }
        List<ByteBuffer> addrsB = addrs.stream().map(FCastDiscoveryListener::addrConvert).collect(Collectors.toList());
        serviceFound(serviceInfo.getServiceName(), addrsB, serviceInfo.getPort());

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            nsdManager.registerServiceInfoCallback(serviceInfo, Runnable::run, new NsdManager.ServiceInfoCallback() {
                @Override
                public void onServiceInfoCallbackRegistrationFailed(int errorCode) {
                }

                @Override
                public void onServiceUpdated(@NonNull NsdServiceInfo serviceInfo) {
                    serviceFound(serviceInfo.getServiceName(), serviceInfo.getHostAddresses().stream().map(FCastDiscoveryListener::addrConvert).collect(Collectors.toList()), serviceInfo.getPort());
                }

                @Override
                public void onServiceLost() {
                    serviceLost(serviceInfo.getServiceName());
                }

                @Override
                public void onServiceInfoCallbackUnregistered() {
                }
            });
        } else {
            nsdManager.resolveService(serviceInfo, new NsdManager.ResolveListener() {
                @Override
                public void onResolveFailed(NsdServiceInfo serviceInfo, int errorCode) {
                    Log.e(TAG, "Service failed to resolve serviceInfo=" + serviceInfo);
                }

                @Override
                public void onServiceResolved(NsdServiceInfo serviceInfo) {
                    Log.i(TAG, "Service resolved serviceInfo=" + serviceInfo);
                    InetAddress addr = serviceInfo.getHost();
                    if (addr != null) {
                        serviceFound(serviceInfo.getServiceName(), List.of(addrConvert(addr)), serviceInfo.getPort());
                    }
                }
            });
        }
    }

    @Override
    public void onServiceLost(NsdServiceInfo serviceInfo) {
        Log.i(TAG, "Service lost serviceInfo=" + serviceInfo);
        serviceLost(serviceInfo.getServiceName());
    }

    private native void serviceFound(String name, List<ByteBuffer> addrs, int port);

    private native void serviceLost(String name);
}

class Discoverer {
    public Discoverer(Context context) {
        NsdManager nsdManager = (NsdManager) context.getSystemService(Context.NSD_SERVICE);
        nsdManager.discoverServices("_fcast._tcp", NsdManager.PROTOCOL_DNS_SD, new FCastDiscoveryListener(nsdManager));
    }
}


public class MainActivity extends NativeActivity implements DisplayManager.DisplayListener {
    private static final int REQUEST_CODE = 1;
    private static final String TAG = "MainActivity";

    static {
        System.loadLibrary("gstreamer_android");
        System.loadLibrary("fcastsender");
    }

    private DisplayManager displayManager;
    private ScreenCaptureCoordinator coordinator;
    private QrScannerLauncher qr;
    private final AtomicBoolean graphSmokeSequenceRan = new AtomicBoolean(false);

    private AppGraph appGraph() {
        return ((FcastApp) getApplication()).getGraph();
    }

    @Override
    public void onBackPressed() {
        Log.d(TAG, "onBackPressed");
        nativeBackPressed();
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        if (event.getKeyCode() == KeyEvent.KEYCODE_BACK && event.getAction() == KeyEvent.ACTION_UP) {
            Log.d(TAG, "dispatchKeyEvent ACTION_UP KEYCODE_BACK");
            nativeBackPressed();
            return true;
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            Log.d(TAG, "onKeyDown KEYCODE_BACK");
            return true;
        }
        return super.onKeyDown(keyCode, event);
    }

    @Override
    public boolean onKeyUp(int keyCode, KeyEvent event) {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            Log.d(TAG, "onKeyUp KEYCODE_BACK");
            nativeBackPressed();
            return true;
        }
        return super.onKeyUp(keyCode, event);
    }

    @Override
    public void onDisplayAdded(int displayId) { }

    @Override
    public void onDisplayRemoved(int displayId) { }

    @Override
    public void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
    }

    @Override
    public void onDisplayChanged(int displayId) { }


    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        try {
            GStreamer.init(this);
        } catch (Exception e) {
            Log.e(TAG, "Failed to init GStreamer ${e}");
            finish();
        }

        Discoverer discoverer = new Discoverer(this);

        coordinator = appGraph().newCaptureCoordinator(
            new ScreenCaptureCoordinator.CaptureCallbacks() {
                @Override public void onPermissionRequested(Intent intent) {
                    startActivityForResult(intent, REQUEST_CODE);
                }
                @Override public void onCaptureStarted(int w, int h)       { nativeCaptureStarted(); }
                @Override public void onCaptureStopped()                    { nativeCaptureStopped(); }
                @Override public void onCaptureCancelled(String reason)     { nativeCaptureCancelled(); }
            }
        );
        coordinator.attach();

        qr = new QrScannerLauncher(this);

        displayManager = (DisplayManager)getSystemService(Context.DISPLAY_SERVICE);
        displayManager.registerDisplayListener(this, new Handler(getMainLooper()));

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                requestPermissions(new String[]{android.Manifest.permission.POST_NOTIFICATIONS}, 101);
            }
        }
    }

    @Override
    protected void onDestroy() {
        if (coordinator != null) {
            coordinator.shutdown();
            coordinator = null;
        }

        if (displayManager != null) {
            try {
                displayManager.unregisterDisplayListener(this);
            } catch (IllegalArgumentException ignored) {
            }
            displayManager = null;
        }

        super.onDestroy();
    }

    @Override
    protected void onStop() {
        // onStop intentionally left out — capture must survive background.
        // See refactor step 03.4 for context.
        super.onStop();
    }

    // Called from native code
    private void startScreenCapture(int scaleWidth, int scaleHeight, int maxFramerate) {
        Log.d(TAG, "Requesting screen capture permissions: scaleWidth=" + scaleWidth + " scaleHeight=" + scaleHeight + " maxFramerate=" + maxFramerate);
        CaptureConfig config = new CaptureConfig(scaleWidth, scaleHeight, maxFramerate);
        coordinator.startCapture(config);
    }

    // Called from native code
    private void stopCapture() {
        coordinator.stopCapture();
    }

    // Called from native code
    private void scanQr() {
        qr.launch();
    }

    // Called from native code
    private void finishApp() {
        runOnUiThread(this::finish);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQUEST_CODE) {
            if (resultCode == RESULT_OK) {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    Intent serviceIntent = new Intent(this, ScreenCaptureService.class);
                    serviceIntent.setAction(ScreenCaptureService.ACTION_RESULT);
                    serviceIntent.putExtra("resultCode", resultCode);
                    serviceIntent.putExtra("data", data);

                    Log.d(TAG, "Starting foreground service SDK=" + Build.VERSION.SDK_INT);

                    try {
                        startForegroundService(serviceIntent);
                    } catch (Exception e) {
                        Log.e(TAG, "Failed to start foreground service: " + e);
                    }
                } else {
                    Log.d(TAG, "Starting capture");
                    CaptureResultBus.deliver(resultCode, data);
                }
            } else if (resultCode == RESULT_CANCELED) {
                Log.d(TAG, "Media projection Canceled");
                CaptureResultBus.deliver(resultCode, new Intent());
            }
        } else if (requestCode == QrScannerLauncher.REQUEST_CODE && resultCode == RESULT_OK) {
            String result = data.getStringExtra("SCAN_RESULT");
            nativeQrScanResult(result);
        }
    }

    public static final class GraphCommandResponse {
        public final boolean success;
        public final String error;
        public final JSONObject info;
        public final JSONObject raw;

        private GraphCommandResponse(boolean success, String error, JSONObject info, JSONObject raw) {
            this.success = success;
            this.error = error;
            this.info = info;
            this.raw = raw;
        }

        public static GraphCommandResponse success(JSONObject info, JSONObject raw) {
            return new GraphCommandResponse(true, null, info, raw);
        }

        public static GraphCommandResponse error(String error, JSONObject raw) {
            return new GraphCommandResponse(false, error, null, raw);
        }
    }

    public GraphCommandResponse graphCommand(String payloadJson) {
        String responseJson = nativeGraphCommand(payloadJson == null ? "" : payloadJson);
        return parseGraphCommandResponse(responseJson);
    }

    public GraphCommandResponse graphCommand(String action, JSONObject params) {
        try {
            JSONObject payload = new JSONObject();
            payload.put(action, params == null ? new JSONObject() : params);
            return graphCommand(payload);
        } catch (JSONException e) {
            return GraphCommandResponse.error("Invalid graph command payload: " + e.getMessage(), null);
        }
    }

    public GraphCommandResponse graphCommand(JSONObject payload) {
        return graphCommand(payload == null ? "" : payload.toString());
    }

    private void logGraphInfo(String phase) {
        GraphCommandResponse response = graphCommand("getinfo", new JSONObject());
        if (!response.success) {
            Log.w(
                    TAG,
                    "Graph getinfo failed phase=" + phase + " error=" + response.error +
                            " raw=" + (response.raw == null ? "null" : response.raw.toString())
            );
            return;
        }

        JSONObject info = response.info;
        JSONObject nodes = info == null ? null : info.optJSONObject("nodes");
        int nodeCount = nodes == null ? 0 : nodes.length();
        Log.i(TAG, "Graph getinfo phase=" + phase + " nodes=" + nodeCount);
    }

    private void runGraphSmokeSequence() {
        if (!graphSmokeSequenceRan.compareAndSet(false, true)) {
            return;
        }

        String suffix = String.valueOf(System.currentTimeMillis());
        String sourceId = "java-smoke-videogen-" + suffix;
        String mixerId = "java-smoke-mixer-" + suffix;
        String linkId = "java-smoke-link-" + suffix;

        boolean sourceCreated = false;
        boolean mixerCreated = false;
        try {
            JSONObject createVideoGeneratorParams = new JSONObject();
            createVideoGeneratorParams.put("id", sourceId);
            GraphCommandResponse createVideoGeneratorResponse =
                    graphCommand("createvideogenerator", createVideoGeneratorParams);
            if (!createVideoGeneratorResponse.success) {
                Log.w(
                        TAG,
                        "Graph smoke createvideogenerator failed id=" + sourceId +
                                " error=" + createVideoGeneratorResponse.error
                );
                return;
            }
            sourceCreated = true;

            JSONObject createMixerParams = new JSONObject();
            createMixerParams.put("id", mixerId);
            createMixerParams.put("audio", false);
            createMixerParams.put("video", true);
            GraphCommandResponse createMixerResponse = graphCommand("createmixer", createMixerParams);
            if (!createMixerResponse.success) {
                Log.w(
                        TAG,
                        "Graph smoke createmixer failed id=" + mixerId + " error=" + createMixerResponse.error
                );
                return;
            }
            mixerCreated = true;

            JSONObject connectParams = new JSONObject();
            connectParams.put("link_id", linkId);
            connectParams.put("src_id", sourceId);
            connectParams.put("sink_id", mixerId);
            connectParams.put("audio", false);
            connectParams.put("video", true);
            GraphCommandResponse connectResponse = graphCommand("connect", connectParams);
            if (!connectResponse.success) {
                Log.w(
                        TAG,
                        "Graph smoke connect failed link=" + linkId + " error=" + connectResponse.error
                );
                return;
            }

            JSONObject startMixerParams = new JSONObject();
            startMixerParams.put("id", mixerId);
            GraphCommandResponse startMixerResponse = graphCommand("start", startMixerParams);
            if (!startMixerResponse.success) {
                Log.w(
                        TAG,
                        "Graph smoke start failed id=" + mixerId + " error=" + startMixerResponse.error
                );
                return;
            }

            JSONObject startSourceParams = new JSONObject();
            startSourceParams.put("id", sourceId);
            GraphCommandResponse startSourceResponse = graphCommand("start", startSourceParams);
            if (!startSourceResponse.success) {
                Log.w(
                        TAG,
                        "Graph smoke start failed id=" + sourceId + " error=" + startSourceResponse.error
                );
                return;
            }

            GraphCommandResponse infoResponse = graphCommand("getinfo", new JSONObject());
            if (!infoResponse.success || infoResponse.info == null) {
                Log.w(
                        TAG,
                        "Graph smoke getinfo failed after graph setup source=" + sourceId +
                                " mixer=" + mixerId +
                                " error=" + infoResponse.error
                );
                return;
            }

            JSONObject nodes = infoResponse.info.optJSONObject("nodes");
            int nodeCount = nodes == null ? 0 : nodes.length();
            boolean sourceFound = nodes != null && nodes.has(sourceId);
            boolean mixerFound = nodes != null && nodes.has(mixerId);
            boolean slotFound = false;
            if (mixerFound) {
                JSONObject mixerInfoWrapper = nodes.optJSONObject(mixerId);
                JSONObject mixerInfo = mixerInfoWrapper == null ? null : mixerInfoWrapper.optJSONObject("mixer");
                JSONObject slots = mixerInfo == null ? null : mixerInfo.optJSONObject("slots");
                slotFound = slots != null && slots.has(linkId);
            }

            Log.i(
                    TAG,
                    "Graph smoke mini-graph source=" + sourceId +
                            " mixer=" + mixerId +
                            " link=" + linkId +
                            " sourceFound=" + sourceFound +
                            " mixerFound=" + mixerFound +
                            " slotFound=" + slotFound +
                            " nodes=" + nodeCount
            );
        } catch (JSONException e) {
            Log.e(TAG, "Graph smoke sequence JSON failure", e);
        } finally {
            if (sourceCreated) {
                try {
                    JSONObject removeSourceParams = new JSONObject();
                    removeSourceParams.put("id", sourceId);
                    GraphCommandResponse removeSourceResponse = graphCommand("remove", removeSourceParams);
                    if (!removeSourceResponse.success) {
                        Log.w(
                                TAG,
                                "Graph smoke remove failed id=" + sourceId + " error=" + removeSourceResponse.error
                        );
                    }
                } catch (JSONException e) {
                    Log.e(TAG, "Graph smoke cleanup source JSON failure", e);
                }
            }
            if (mixerCreated) {
                try {
                    JSONObject removeMixerParams = new JSONObject();
                    removeMixerParams.put("id", mixerId);
                    GraphCommandResponse removeMixerResponse = graphCommand("remove", removeMixerParams);
                    if (!removeMixerResponse.success) {
                        Log.w(
                                TAG,
                                "Graph smoke remove failed id=" + mixerId + " error=" + removeMixerResponse.error
                        );
                    }
                } catch (JSONException e) {
                    Log.e(TAG, "Graph smoke cleanup mixer JSON failure", e);
                }
            }
        }
    }

    private GraphCommandResponse parseGraphCommandResponse(String responseJson) {
        if (responseJson == null || responseJson.isEmpty()) {
            return GraphCommandResponse.error("Empty graph command response", null);
        }

        try {
            JSONObject root = new JSONObject(responseJson);
            Object result = root.opt("result");
            if (result instanceof String) {
                String resultName = (String) result;
                if ("success".equals(resultName)) {
                    return GraphCommandResponse.success(null, root);
                }
                return GraphCommandResponse.error(
                        "Unexpected graph result string: " + resultName,
                        root
                );
            }

            if (result instanceof JSONObject) {
                JSONObject resultObject = (JSONObject) result;
                if (resultObject.has("error")) {
                    return GraphCommandResponse.error(
                            resultObject.optString("error", "Unknown graph command error"),
                            root
                    );
                }
                if (resultObject.has("info")) {
                    JSONObject info = resultObject.optJSONObject("info");
                    return GraphCommandResponse.success(info, root);
                }
            }

            return GraphCommandResponse.error("Unsupported graph result shape", root);
        } catch (JSONException e) {
            Log.e(TAG, "Failed to parse graph command response: " + responseJson, e);
            return GraphCommandResponse.error("Invalid graph response JSON: " + e.getMessage(), null);
        }
    }

    public static native void nativeProcessFrame(int width, int height, ByteBuffer bufferY, ByteBuffer bufferU, ByteBuffer bufferV);

    public static native String nativeGraphCommand(String payloadJson);

    native void nativeCaptureStarted();

    native void nativeCaptureStopped();

    native void nativeCaptureCancelled();

    native void nativeQrScanResult(String result);

    native void nativeBackPressed();
}
