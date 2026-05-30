package org.fcast.android.sender.discovery

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.util.Log
import java.net.Inet6Address
import java.net.InetAddress
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.stream.Collectors

internal class FCastDiscoveryListener(private val nsdManager: NsdManager) : NsdManager.DiscoveryListener {

    override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
        Log.e(TAG, "Failed to start discovery errorCode=$errorCode")
    }

    override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
        Log.e(TAG, "Failed to stop discovery errorCode=$errorCode")
    }

    override fun onDiscoveryStarted(serviceType: String) {
        Log.i(TAG, "Discovery started")
    }

    override fun onDiscoveryStopped(serviceType: String) {
        Log.i(TAG, "Discovery stopped")
    }

    override fun onServiceFound(serviceInfo: NsdServiceInfo) {
        Log.i(TAG, "Service found serviceInfo=$serviceInfo")

        var addrs: List<InetAddress> = emptyList()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            addrs = serviceInfo.hostAddresses
        } else {
            val hostAddr = serviceInfo.host
            if (hostAddr != null) addrs = listOf(hostAddr)
        }
        val addrsB = addrs.stream().map { addrConvert(it) }.collect(Collectors.toList())
        serviceFound(serviceInfo.serviceName, addrsB, serviceInfo.port)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            nsdManager.registerServiceInfoCallback(serviceInfo, Runnable::run, object : NsdManager.ServiceInfoCallback {
                override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {}
                override fun onServiceUpdated(updated: NsdServiceInfo) {
                    serviceFound(
                        updated.serviceName,
                        updated.hostAddresses.stream().map { addrConvert(it) }.collect(Collectors.toList()),
                        updated.port,
                    )
                }
                override fun onServiceLost() { serviceLost(serviceInfo.serviceName) }
                override fun onServiceInfoCallbackUnregistered() {}
            })
        } else {
            nsdManager.resolveService(serviceInfo, object : NsdManager.ResolveListener {
                override fun onResolveFailed(si: NsdServiceInfo, errorCode: Int) {
                    Log.e(TAG, "Service failed to resolve serviceInfo=$si")
                }
                override fun onServiceResolved(si: NsdServiceInfo) {
                    Log.i(TAG, "Service resolved serviceInfo=$si")
                    val addr = si.host
                    if (addr != null) {
                        serviceFound(si.serviceName, listOf(addrConvert(addr)), si.port)
                    }
                }
            })
        }
    }

    override fun onServiceLost(serviceInfo: NsdServiceInfo) {
        Log.i(TAG, "Service lost serviceInfo=$serviceInfo")
        serviceLost(serviceInfo.serviceName)
    }

    private external fun serviceFound(name: String, addrs: List<ByteBuffer>, port: Int)
    private external fun serviceLost(name: String)

    companion object {
        private const val TAG = "FCastDiscoveryListener"

        private fun addrConvert(addr: InetAddress): ByteBuffer {
            val addrB = addr.address
            val buffer = ByteBuffer.allocateDirect(addrB.size)
            buffer.put(addrB)
            if (addr is Inet6Address) {
                buffer.order(ByteOrder.LITTLE_ENDIAN).putInt(addr.scopeId)
            }
            return buffer
        }
    }
}

internal class Discoverer(context: Context) {
    init {
        val nsdManager = context.getSystemService(Context.NSD_SERVICE) as NsdManager
        nsdManager.discoverServices("_fcast._tcp", NsdManager.PROTOCOL_DNS_SD, FCastDiscoveryListener(nsdManager))
    }
}
