package com.clipsync.net

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-logic tests for [NetworkChangeObserver] behaviour.
 *
 * We cannot instantiate [NetworkChangeObserver] directly in a JVM test
 * because it depends on [android.net.ConnectivityManager] (an Android
 * system service). Instead we test the callback contract via a lightweight
 * simulation of the expected state transitions.
 */
class NetworkChangeObserverTest {

    /**
     * Model the observer's core state machine: track current network ID
     * and decide when to fire reconnect.
     */
    private class ObserverStateMachine {
        var currentNetworkId: Int? = null
        var reconnectCount = 0

        fun onAvailable(networkId: Int) {
            val prev = currentNetworkId
            currentNetworkId = networkId
            if (prev != null && prev != networkId) {
                // Network switched
                reconnectCount++
            } else if (prev == null) {
                // Network restored from nothing
                reconnectCount++
            }
        }

        fun onLost(networkId: Int) {
            if (currentNetworkId == networkId) {
                currentNetworkId = null
            }
        }
    }

    @Test
    fun `first network available triggers reconnect`() {
        val sm = ObserverStateMachine()
        sm.onAvailable(1)
        assertEquals(1, sm.reconnectCount)
        assertEquals(1, sm.currentNetworkId)
    }

    @Test
    fun `same network available again does not trigger reconnect`() {
        val sm = ObserverStateMachine()
        sm.onAvailable(1)
        sm.onAvailable(1)
        assertEquals(1, sm.reconnectCount)
    }

    @Test
    fun `switching network triggers reconnect`() {
        val sm = ObserverStateMachine()
        sm.onAvailable(1) // +1
        sm.onAvailable(2) // +1 (switched)
        assertEquals(2, sm.reconnectCount)
        assertEquals(2, sm.currentNetworkId)
    }

    @Test
    fun `network lost then restored triggers reconnect`() {
        val sm = ObserverStateMachine()
        sm.onAvailable(1)   // +1
        sm.onLost(1)        // no reconnect
        sm.onAvailable(2)   // +1 (from null)
        assertEquals(2, sm.reconnectCount)
    }

    @Test
    fun `losing a different network does not clear current`() {
        val sm = ObserverStateMachine()
        sm.onAvailable(1)
        sm.onLost(999) // unrelated network
        assertEquals(1, sm.currentNetworkId)
    }

    @Test
    fun `multiple switches track correctly`() {
        val sm = ObserverStateMachine()
        sm.onAvailable(1) // +1
        sm.onAvailable(2) // +1
        sm.onAvailable(3) // +1
        sm.onLost(3)
        sm.onAvailable(4) // +1 (from null)
        assertEquals(4, sm.reconnectCount)
    }
}
