package org.fcast.android.sender;

import static org.junit.Assert.assertEquals;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.Mockito.spy;
import static org.mockito.Mockito.verify;

import android.app.Notification;
import android.app.Service;
import android.content.Intent;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.Robolectric;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

/**
 * Validates {@link ScreenCaptureService#onStartCommand} contract added in
 * refactor step 01 (action-constant guard) and step 03 (lifecycle cleanup).
 *
 * Robolectric stubs system services; this test is JVM-only and does not
 * touch the emulator.
 */
@RunWith(RobolectricTestRunner.class)
@Config(sdk = 34, application = android.app.Application.class)
public class ScreenCaptureServiceTest {

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
        ScreenCaptureService svc = spy(Robolectric.setupService(ScreenCaptureService.class));
        Intent data = new Intent();
        Intent intent = new Intent()
            .setAction(ScreenCaptureService.ACTION_RESULT)
            .putExtra("resultCode", android.app.Activity.RESULT_OK)
            .putExtra("data", data);
        int result = svc.onStartCommand(intent, 0, 1);
        verify(svc).startForeground(anyInt(), any(Notification.class));
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void cancelledResult_doesNotStartForeground() {
        ScreenCaptureService svc = spy(Robolectric.setupService(ScreenCaptureService.class));
        Intent intent = new Intent()
            .setAction(ScreenCaptureService.ACTION_RESULT)
            .putExtra("resultCode", android.app.Activity.RESULT_CANCELED);
        int result = svc.onStartCommand(intent, 0, 1);
        // Never started → no call.
        verify(svc, org.mockito.Mockito.never()).startForeground(anyInt(), any(Notification.class));
        assertEquals(Service.START_NOT_STICKY, result);
    }

    @Test
    public void notificationChannel_isCreatedOnFirstStart() {
        ScreenCaptureService svc = Robolectric.setupService(ScreenCaptureService.class);
        android.app.NotificationManager nm =
            (android.app.NotificationManager) svc.getSystemService(
                android.content.Context.NOTIFICATION_SERVICE);
        // The channel ID is part of the service contract; mirror it here.
        // If you prefer not to hardcode, expose the constant from the service.
        assert nm != null;
        assert nm.getNotificationChannel(
            "org.fcast.android.sender.ScreenCaptureService") != null;
    }
}
