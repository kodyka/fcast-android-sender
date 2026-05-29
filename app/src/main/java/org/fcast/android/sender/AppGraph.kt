package org.fcast.android.sender

import android.content.Context
import org.fcast.android.sender.capture.CaptureEngine
import org.fcast.android.sender.capture.ScreenCaptureCoordinator
import org.fcast.android.sender.data.AndroidSecretStore
import org.fcast.android.sender.data.SecretStore
import org.fcast.android.sender.runtime.JniRuntimeBridge
import org.fcast.android.sender.runtime.RuntimeBridge

/**
 * Production composition root.
 *
 * Wires every long-lived dependency the application needs. One instance per
 * process; constructed in FcastApp.onCreate. Activities and services read
 * dependencies from here rather than constructing them ad-hoc.
 *
 * Replaces:
 *   - Static calls to GstPopServiceBridge / MigrationRuntimeServiceBridge.
 *   - The process-global Rust BACKEND singleton (via JniRuntimeBridge).
 */
class AppGraph(
    private val appContext: Context,
) {
    val runtime: RuntimeBridge by lazy {
        JniRuntimeBridge(appContext)
    }

    val secretStore: SecretStore by lazy {
        AndroidSecretStore(appContext)
    }

    /** The coordinator is created lazily so it doesn't bind GL state at process start. */
    fun newCaptureCoordinator(
        callbacks: ScreenCaptureCoordinator.CaptureCallbacks,
    ): ScreenCaptureCoordinator = org.fcast.android.sender.capture.RealScreenCaptureCoordinator(
        applicationContext = appContext,
        callbacks = callbacks,
        engineFactory = { CaptureEngine() },
    )
}
