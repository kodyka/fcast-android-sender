package org.fcast.android.sender

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.runBlocking
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.runtime.JniRuntimeBridge
import org.junit.Assert.assertNotNull
import org.junit.BeforeClass
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Verifies that the real JNI library loads, the bridge is constructable,
 * and a status query round-trips through Rust without crashing.
 *
 * Runs on an emulator (or device) via :app:connectedDebugAndroidTest.
 */
@RunWith(AndroidJUnit4::class)
class RuntimeBridgeInstrumentedTest {

    companion object {
        @BeforeClass @JvmStatic
        fun loadNativeLibraries() {
            // Mirror MainActivity's load order: fcastsender links against gstreamer_android,
            // so gstreamer_android must be loaded first or dlopen fails.
            System.loadLibrary("gstreamer_android")
            System.loadLibrary("fcastsender")
        }
    }

    @Test
    fun jniLibrary_isLoadable() {
        // Verifies both libraries are already loaded (by @BeforeClass) without error.
        // A duplicate loadLibrary call is a no-op; this test is a canary for load-order regression.
        System.loadLibrary("gstreamer_android")
        System.loadLibrary("fcastsender")
    }

    @Test
    fun statusPing_returnsParseableJson_forMigration() = runBlocking {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val bridge = JniRuntimeBridge(ctx)
        val status = bridge.backendStatus(BackendKind.MIGRATION)
        assertNotNull(status.state)
    }

    @Test
    fun statusPing_returnsParseableJson_forGstpop() = runBlocking {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val bridge = JniRuntimeBridge(ctx)
        val status = bridge.backendStatus(BackendKind.GSTPOP)
        assertNotNull(status.state)
    }

    @Test
    fun startThenStop_doesNotCrash_forMigration() = runBlocking {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val bridge = JniRuntimeBridge(ctx)
        val started = bridge.startEmbeddedBackend(BackendKind.MIGRATION, "{}")
        assertNotNull(started.state)
        val stopped = bridge.stopEmbeddedBackend(BackendKind.MIGRATION)
        assertNotNull(stopped.state)
    }
}
