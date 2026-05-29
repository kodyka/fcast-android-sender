package org.fcast.android.sender

import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ScreenCaptureServiceInstrumentedTest {

    @Test
    fun unknownAction_doesNotCrashTheProcess() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val intent = Intent(ctx, ScreenCaptureService::class.java).setAction("unknown")
        // The negative assertion is: control returns from startService.
        ctx.startService(intent)
        Thread.sleep(500) // Let the service deliver the start command.
        // No exception, no native crash.
    }

    @Test
    fun notificationChannel_isRegisteredAfterFirstStart() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val intent = Intent(ctx, ScreenCaptureService::class.java).setAction("noop")
        ctx.startService(intent)
        Thread.sleep(500)

        val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val ch = nm.getNotificationChannel("org.fcast.android.sender.ScreenCaptureService")
        assertNotNull(ch)
    }

    @Test
    fun manifestForegroundServiceType_isMediaProjection() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val pm = ctx.packageManager
        val cn = android.content.ComponentName(ctx, ScreenCaptureService::class.java)
        val info = pm.getServiceInfo(cn, 0)
        // foregroundServiceType is a bitmask; expect MEDIA_PROJECTION = 32.
        val expected = android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
        assert((info.foregroundServiceType and expected) != 0)
    }
}
