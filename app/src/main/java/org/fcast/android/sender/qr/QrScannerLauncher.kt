package org.fcast.android.sender.qr

import android.app.Activity
import com.journeyapps.barcodescanner.ScanOptions

interface QrScannerLauncher {
    fun launch()

    companion object {
        const val REQUEST_CODE = 2
    }
}

/**
 * Wraps the ZXing scanner intent launch.
 *
 * The activity routes the result back via onActivityResult with [REQUEST_CODE].
 * Uses startActivityForResult because MainActivity extends NativeActivity,
 * which does not support ActivityResultLauncher (requires ComponentActivity).
 */
class RealQrScannerLauncher(private val activity: Activity) : QrScannerLauncher {

    @Suppress("DEPRECATION")
    override fun launch() {
        val options = ScanOptions()
            .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
            .setOrientationLocked(true)
            .setBeepEnabled(false)
        activity.startActivityForResult(options.createScanIntent(activity), QrScannerLauncher.REQUEST_CODE)
    }
}
