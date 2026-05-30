package org.fcast.android.sender

import android.app.Application

import org.fcast.android.sender.data.SecretStoreBridge

/**
 * Single Application subclass; owner of the production [AppGraph].
 *
 * Activities and services read dependencies via:
 *
 *     val graph = (applicationContext as FcastApp).graph
 *     graph.runtime.startEmbeddedBackend(...)
 *
 * Do NOT add any other state here; everything goes through AppGraph.
 */
class FcastApp : Application() {
    val graph: AppGraph by lazy { AppGraph(applicationContext) }

    override fun onCreate() {
        super.onCreate()
        SecretStoreBridge.install(graph.secretStore)
    }
}
