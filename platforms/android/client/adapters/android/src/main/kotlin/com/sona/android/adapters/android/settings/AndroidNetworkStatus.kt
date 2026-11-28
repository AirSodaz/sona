package com.sona.android.adapters.android.settings

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import com.sona.android.application.settings.NetworkStatusPort

class AndroidNetworkStatus(
    context: Context,
) : NetworkStatusPort {
    private val connectivityManager =
        context.applicationContext.getSystemService(ConnectivityManager::class.java)

    override fun isActiveNetworkMetered(): Boolean {
        val manager = connectivityManager ?: return false
        val network = manager.activeNetwork ?: return manager.isActiveNetworkMetered
        val capabilities = manager.getNetworkCapabilities(network)
        if (capabilities != null) {
            return !capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
        }
        return manager.isActiveNetworkMetered
    }
}
