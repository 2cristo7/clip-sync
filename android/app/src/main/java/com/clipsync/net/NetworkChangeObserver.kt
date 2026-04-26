package com.clipsync.net

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import com.clipsync.util.L

/**
 * Observes connectivity changes via [ConnectivityManager.NetworkCallback]
 * and invokes [onReconnectNeeded] when the default network changes while
 * the device still has internet.
 *
 * Typical wiring (inside [ClipForegroundService]):
 * ```kotlin
 * val observer = NetworkChangeObserver(this) { reconnect() }
 * observer.register()
 * // on destroy:
 * observer.unregister()
 * ```
 */
class NetworkChangeObserver(
    private val context: Context,
    private val onReconnectNeeded: () -> Unit
) {
    private val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private var callback: ConnectivityManager.NetworkCallback? = null
    private var currentNetwork: Network? = null
    private var registered = false

    /**
     * Start listening for network changes. Builds a [NetworkRequest] for
     * INTERNET-capable transports and registers a callback.
     */
    fun register() {
        if (registered) return
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()

        val cb = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                val prev = currentNetwork
                currentNetwork = network
                L.event(M, "Network available: $network (prev=$prev)")
                if (prev != null && prev != network) {
                    // Switched to a different network (e.g. Wi-Fi → mobile)
                    L.event(M, "Network switched, triggering reconnect")
                    onReconnectNeeded()
                } else if (prev == null) {
                    // Went from no-network to having a network
                    L.event(M, "Network restored, triggering reconnect")
                    onReconnectNeeded()
                }
            }

            override fun onLost(network: Network) {
                L.event(M, "Network lost: $network")
                if (currentNetwork == network) {
                    currentNetwork = null
                }
            }

            override fun onCapabilitiesChanged(
                network: Network,
                caps: NetworkCapabilities
            ) {
                L.verbose(M, "Capabilities changed: $network validated=${caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)}")
            }
        }
        cm.registerNetworkCallback(request, cb)
        callback = cb
        registered = true
        L.event(M, "NetworkChangeObserver registered")
    }

    /**
     * Stop listening. Safe to call even if never registered.
     */
    fun unregister() {
        if (!registered) return
        callback?.let {
            try {
                cm.unregisterNetworkCallback(it)
            } catch (_: Throwable) {
                // Already unregistered or context destroyed
            }
        }
        callback = null
        currentNetwork = null
        registered = false
        L.event(M, "NetworkChangeObserver unregistered")
    }

    companion object {
        private const val M = "Net"
    }
}
