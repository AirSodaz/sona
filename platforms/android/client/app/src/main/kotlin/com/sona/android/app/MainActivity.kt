package com.sona.android.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel

class MainActivity : ComponentActivity() {
    private val container by lazy { SonaAppContainer() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val sonaViewModel: SonaViewModel = viewModel(
                factory = SonaViewModel.factory(container.loadSonaBootstrap),
            )
            val bootstrapState by sonaViewModel.bootstrapState.collectAsStateWithLifecycle()
            SonaApp(
                bootstrapState = bootstrapState,
                onRetryBootstrap = sonaViewModel::refresh,
            )
        }
    }
}
