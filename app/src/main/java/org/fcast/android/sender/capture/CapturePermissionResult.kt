package org.fcast.android.sender.capture

import android.content.Intent

/**
 * Outcome of the MediaProjection consent dialog (presented to the user the
 * first time they tap "start cast").
 *
 * Replaces the previous Bundle/Intent passing of `resultCode` + `data` that
 * leaked into half of MainActivity.
 */
sealed class CapturePermissionResult {
    /** The user tapped "Start now" — the engine can proceed. */
    data class Granted(val resultCode: Int, val data: Intent) : CapturePermissionResult()

    /** The user dismissed or cancelled the dialog. */
    data object Cancelled : CapturePermissionResult()

    /** Some other failure path (e.g. an `Intent.parseUri` error). */
    data class Failed(val reason: String, val cause: Throwable? = null) : CapturePermissionResult()
}
