package com.sona.android.app.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState

@Composable
internal fun SettingsScreen(
    bootstrapState: SonaBootstrapUiState,
    dynamicColorEnabled: Boolean,
    onDynamicColorChanged: (Boolean) -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp, vertical = 20.dp),
        verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Text(
            text = stringResource(R.string.appearance_heading),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(stringResource(R.string.dynamic_color))
            Switch(
                checked = dynamicColorEnabled,
                onCheckedChange = onDynamicColorChanged,
            )
        }
        HorizontalDivider()
        Text(
            text = stringResource(R.string.runtime_heading),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        RuntimeStatus(bootstrapState)
    }
}

@Composable
private fun RuntimeStatus(bootstrapState: SonaBootstrapUiState) {
    val text = when (bootstrapState) {
        SonaBootstrapUiState.Loading -> stringResource(R.string.status_loading)
        is SonaBootstrapUiState.Error -> stringResource(R.string.status_error)
        is SonaBootstrapUiState.Ready -> if (bootstrapState.snapshot.localRuntimePackaged) {
            stringResource(R.string.local_runtime_ready)
        } else {
            stringResource(R.string.local_runtime_unavailable)
        }
    }
    Text(
        text = text,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
