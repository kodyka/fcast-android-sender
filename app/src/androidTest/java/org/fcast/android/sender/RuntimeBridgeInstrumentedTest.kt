package org.fcast.android.sender

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.fcast.android.sender.runtime.BackendKind
import org.fcast.android.sender.runtime.JniRuntimeBridge
import org.junit.Assert.assertNotNull
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

    @Test
    fun jniLibrary_isLoadable() {
        // System.loadLibrary("fcastsender") runs in MainActivity's static block.
        // Here we explicitly trigger it again; the second call is a no-op.
        System.loadLibrary("fcastsender")
    }

    @Test
    fun statusPing_returnsParseableJson_forMigration() = runBlocking {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val bridge = JniRuntimeBridge(ctx)
        val status = bridge.backendStatus(BackendKind.MIGRATION)
        assertNotNull(status.state)
    }

    @Test
    fun statusPing_returnsParseableJson_forGstpop() = runBlocking {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val bridge = JniRuntimeBridge(ctx)
        val status = bridge.backendStatus(BackendKind.GSTPOP)
        assertNotNull(status.state)
    }

    @Test
    fun startThenStop_doesNotCrash_forMigration() = runBlocking {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val bridge = JniRuntimeBridge(ctx)
        val started = bridge.startEmbeddedBackend(BackendKind.MIGRATION, "{}")
        assertNotNull(started.state)
        val stopped = bridge.stopEmbeddedBackend(BackendKind.MIGRATION)
        assertNotNull(stopped.state)
    }
}
