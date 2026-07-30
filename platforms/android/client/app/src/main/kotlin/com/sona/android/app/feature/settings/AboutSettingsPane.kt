package com.sona.android.app.feature.settings

import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.OpenInNew
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.SystemUpdate
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.settings.AppUpdateChannel

private const val PROJECT_URL = "https://github.com/AirSodaz/sona"

@Composable
internal fun AboutSettingsPane(
    state: AboutSettingsUiState,
    onShown: () -> Unit,
    onCheckForUpdates: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val uriHandler = LocalUriHandler.current
    LaunchedEffect(Unit) { onShown() }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 24.dp, vertical = 20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = 720.dp)
                .fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Image(
                    painter = painterResource(R.mipmap.ic_launcher),
                    contentDescription = null,
                    modifier = Modifier.size(72.dp),
                )
                Spacer(Modifier.width(16.dp))
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(
                        text = state.build.appName,
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        text = stringResource(
                            R.string.about_version_value,
                            state.build.versionName,
                            state.build.versionCode,
                        ),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        text = stringResource(
                            when (state.build.channel) {
                                AppUpdateChannel.STABLE -> R.string.about_channel_stable
                                AppUpdateChannel.NIGHTLY -> R.string.about_channel_nightly
                            },
                        ),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
            }

            HorizontalDivider()
            Text(
                text = stringResource(R.string.about_update_heading),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            UpdateStatus(
                status = state.updateStatus,
                onCheckForUpdates = onCheckForUpdates,
                onOpenRelease = { url -> uriHandler.openUri(url) },
            )

            HorizontalDivider()
            ListItem(
                headlineContent = { Text(stringResource(R.string.about_project_page)) },
                supportingContent = { Text(PROJECT_URL) },
                leadingContent = {
                    Icon(Icons.Rounded.OpenInNew, contentDescription = null)
                },
                trailingContent = {
                    Icon(
                        Icons.Rounded.OpenInNew,
                        contentDescription = stringResource(R.string.about_open_external),
                    )
                },
                colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { uriHandler.openUri(PROJECT_URL) },
            )
        }
    }
}

@Composable
private fun UpdateStatus(
    status: AboutUpdateStatus,
    onCheckForUpdates: () -> Unit,
    onOpenRelease: (String) -> Unit,
) {
    val icon = when (status) {
        AboutUpdateStatus.Idle -> Icons.Rounded.SystemUpdate
        AboutUpdateStatus.Checking -> null
        is AboutUpdateStatus.UpToDate -> Icons.Rounded.CheckCircle
        is AboutUpdateStatus.UpdateAvailable -> Icons.Rounded.SystemUpdate
        AboutUpdateStatus.Error -> Icons.Rounded.ErrorOutline
    }
    val message = when (status) {
        AboutUpdateStatus.Idle -> R.string.about_update_idle
        AboutUpdateStatus.Checking -> R.string.about_update_checking
        is AboutUpdateStatus.UpToDate -> R.string.about_update_current
        is AboutUpdateStatus.UpdateAvailable -> R.string.about_update_available
        AboutUpdateStatus.Error -> R.string.about_update_failed
    }

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (status is AboutUpdateStatus.Checking) {
                CircularProgressIndicator(modifier = Modifier.size(24.dp), strokeWidth = 2.dp)
            } else if (icon != null) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = if (status is AboutUpdateStatus.Error) {
                        MaterialTheme.colorScheme.error
                    } else {
                        MaterialTheme.colorScheme.primary
                    },
                )
            }
            Spacer(Modifier.width(16.dp))
            Text(
                text = if (status is AboutUpdateStatus.UpdateAvailable) {
                    stringResource(message, status.release.versionName)
                } else {
                    stringResource(message)
                },
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.weight(1f),
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (status is AboutUpdateStatus.UpdateAvailable) {
                TextButton(onClick = { onOpenRelease(status.release.releasePageUrl) }) {
                    Text(stringResource(R.string.about_view_update))
                    Spacer(Modifier.width(8.dp))
                    Icon(Icons.Rounded.OpenInNew, contentDescription = null)
                }
            }
            Spacer(Modifier.width(8.dp))
            FilledTonalButton(
                onClick = onCheckForUpdates,
                enabled = status !is AboutUpdateStatus.Checking,
            ) {
                Icon(Icons.Rounded.Refresh, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.about_check_updates))
            }
        }
    }
    Spacer(Modifier.height(4.dp))
}
