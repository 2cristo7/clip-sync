package com.clipsync.discovery

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.util.Log
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import java.net.InetAddress

data class Discovered(
    val host: String,
    val port: Int,
    val fp: String?,      // base64url-nopad SPKI-SHA256 from TXT record, may be null if missing
    val name: String,
    val version: String? = null
)

/**
 * NsdManager-backed mDNS discovery of `_clipsync._tcp` services.
 *
 * Emits a new [Discovered] each time a service is fully resolved. Consumers
 * are responsible for deduplicating by `name` if they want.
 */
class NsdDiscovery(context: Context) {

    private val nsd = context.getSystemService(Context.NSD_SERVICE) as NsdManager

    fun discover(): Flow<Discovered> = callbackFlow {
        val resolveListener = object : NsdManager.ResolveListener {
            override fun onServiceResolved(info: NsdServiceInfo) {
                val addr = info.host ?: return
                val port = info.port
                val parsed = parseTxt(info.attributes)
                val host = resolveHost(addr)
                val d = Discovered(
                    host = host,
                    port = port,
                    fp = parsed["fp"],
                    name = info.serviceName ?: "",
                    version = parsed["version"]
                )
                Log.i(TAG, "Resolved $d")
                trySend(d)
            }

            override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) {
                Log.w(TAG, "Resolve failed for ${info.serviceName}: $errorCode")
            }
        }

        val discoveryListener = object : NsdManager.DiscoveryListener {
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.e(TAG, "Start discovery failed: $errorCode")
                close()
            }

            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.w(TAG, "Stop discovery failed: $errorCode")
            }

            override fun onDiscoveryStarted(serviceType: String) {
                Log.i(TAG, "Discovery started for $serviceType")
            }

            override fun onDiscoveryStopped(serviceType: String) {
                Log.i(TAG, "Discovery stopped")
            }

            override fun onServiceFound(info: NsdServiceInfo) {
                Log.i(TAG, "Found ${info.serviceName} / ${info.serviceType}")
                @Suppress("DEPRECATION")
                nsd.resolveService(info, resolveListener)
            }

            override fun onServiceLost(info: NsdServiceInfo) {
                Log.i(TAG, "Lost ${info.serviceName}")
            }
        }

        nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discoveryListener)

        awaitClose {
            try {
                nsd.stopServiceDiscovery(discoveryListener)
            } catch (t: Throwable) {
                Log.w(TAG, "stopServiceDiscovery: ${t.message}")
            }
        }
    }

    companion object {
        private const val TAG = "ClipSync/NSD"
        const val SERVICE_TYPE = "_clipsync._tcp."

        private fun resolveHost(addr: InetAddress): String = addr.hostAddress ?: addr.hostName ?: ""

        /**
         * Convert a raw NSD TXT attribute map (byte[] values) into strings.
         * Public so unit tests can exercise it without instantiating [NsdDiscovery].
         */
        fun parseTxt(attrs: Map<String, ByteArray?>?): Map<String, String> {
            if (attrs == null) return emptyMap()
            val out = HashMap<String, String>(attrs.size)
            for ((k, v) in attrs) {
                if (v != null) out[k] = String(v, Charsets.UTF_8)
            }
            return out
        }
    }
}
