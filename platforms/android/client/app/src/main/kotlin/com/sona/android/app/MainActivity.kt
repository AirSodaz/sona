package com.sona.android.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sona.android.app.composition.SonaAppContainer
import com.sona.android.app.feature.bootstrap.SonaBootstrapViewModel
import com.sona.android.app.navigation.SonaApp

class MainActivity : ComponentActivity() {
    private val container by lazy { SonaAppContainer() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val bootstrapViewModel: SonaBootstrapViewModel = viewModel(
                factory = SonaBootstrapViewModel.factory(container.loadSonaBootstrap),
            )
            val bootstrapState by bootstrapViewModel.bootstrapState.collectAsStateWithLifecycle()
            SonaApp(
                bootstrapState = bootstrapState,
                onRetryBootstrap = bootstrapViewModel::refresh,
            )
        }
    }
}
