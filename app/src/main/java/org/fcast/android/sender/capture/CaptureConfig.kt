package org.fcast.android.sender.capture

/**
 * User-controlled capture parameters supplied by the Slint UI when the user
 * starts casting.
 *
 * @param scaleWidth Target output width in pixels. Zero / negative means
 *   "use the display's native width unchanged".
 * @param scaleHeight Target output height in pixels. Zero / negative means
 *   "use the display's native height unchanged".
 * @param maxFps Maximum number of frames per second to push through the
 *   engine. The engine drops frames that arrive faster than this. Must be > 0.
 *
 * Instances are intended to be created once per capture session and not
 * mutated. Use [copy] to derive variations.
 */
data class CaptureConfig(
    val scaleWidth: Int,
    val scaleHeight: Int,
    val maxFps: Int,
) {
    init {
        require(maxFps > 0) { "maxFps must be > 0, got $maxFps" }
    }

    val minIntervalNanos: Long
        get() = 1_000_000_000L / maxFps

    fun targetWidth(displayWidth: Int): Int =
        if (scaleWidth > 0) scaleWidth else displayWidth

    fun targetHeight(displayHeight: Int): Int =
        if (scaleHeight > 0) scaleHeight else displayHeight
}
