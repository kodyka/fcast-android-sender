package org.fcast.android.sender;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.robolectric.Shadows.shadowOf;

import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;

import org.junit.After;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.Robolectric;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import org.robolectric.shadows.ShadowService;

/**
 * Validates {@link ScreenCaptureService#onStartCommand} contract added in
 * refactor step 01 (action-constant guard) and step 03 (lifecycle cleanup).
 *
 * Robolectric stubs system services; this test is JVM-only and does not
 * touch the emulator.
 */
@RunWith(RobolectricTestRunner.class)
// application=Application.class: test manifest has no custom Application subclass; avoids class-not-found on init
@Config(sdk = 34, application = android.app.Application.class)
public class ScreenCaptureServiceTest {

    @After
    public void tearDown() {
        // Reset singleton so tests cannot leak a listener into the next test.
        CaptureResultBus.setListener(null);
    }

    @Test
    public void nullIntent_doesNotCrash_andReturnsNotSticky() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        int result = svc.onStartCommand(null, 0, 1);
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void unknownAction_isIgnored_andReturnsNotSticky() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        Intent intent = new Intent().setAction("unknown");
        int result = svc.onStartCommand(intent, 0, 1);
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void grantedResult_callsStartForeground() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        Intent data = new Intent();
        Intent intent = new Intent()
            .setAction(ScreenCaptureService.ACTION_RESULT)
            .putExtra("resultCode", android.app.Activity.RESULT_OK)
            .putExtra("data", data);
        int result = svc.onStartCommand(intent, 0, 1);
        ShadowService shadow = shadowOf(svc);
        assertNotNull(shadow.getLastForegroundNotification());
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void cancelledResult_doesNotStartForeground() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        Intent intent = new Intent()
            .setAction(ScreenCaptureService.ACTION_RESULT)
            .putExtra("resultCode", android.app.Activity.RESULT_CANCELED);
        int result = svc.onStartCommand(intent, 0, 1);
        ShadowService shadow = shadowOf(svc);
        assertNull(shadow.getLastForegroundNotification());
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void notificationChannel_isCreatedOnFirstStart() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        NotificationManager nm =
            (NotificationManager) svc.getSystemService(Context.NOTIFICATION_SERVICE);
        assertNotNull(nm);
        assertNotNull(nm.getNotificationChannel("org.fcast.android.sender.ScreenCaptureService"));
    }
}
