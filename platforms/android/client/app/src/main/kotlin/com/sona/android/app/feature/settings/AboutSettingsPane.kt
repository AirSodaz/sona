package com.sona.android.app.feature.settings

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.InstallMobile
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.SystemUpdate
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.ui.component.InsetGroupedCard
import com.sona.android.app.ui.component.SonaOptionItem
import com.sona.android.app.ui.component.SonaSingleChoiceDialog
import com.sona.android.app.ui.component.TwoLineItemDivider
import com.sona.android.app.ui.component.TwoLineItemRow
import com.sona.android.app.ui.component.springOverscroll
import com.sona.android.application.settings.AppUpdateChannel
import com.sona.android.application.settings.AppUpdateErrorCategory
import com.sona.android.application.settings.DownloadMirror

private const val PROJECT_URL = "https://github.com/AirSodaz/sona"

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AboutSettingsPane(
    state: AboutSettingsUiState,
    onShown: () -> Unit,
    onCheckForUpdates: () -> Unit,
    modifier: Modifier = Modifier,
    onDownloadUpdate: () -> Unit = {},
    onCancelDownload: () -> Unit = {},
    onInstallUpdate: () -> Unit = {},
    onIgnoreUpdate: () -> Unit = {},
    onClearIgnoredUpdate: () -> Unit = {},
    onConfirmCellularDownload: () -> Unit = {},
    onDismissCellularDownload: () -> Unit = {},
    onOpenInstallPermissionSettings: () -> Unit = {},
    onDismissInstallPermissionDialog: () -> Unit = {},
    onRetryWithMirror: (DownloadMirror) -> Unit = {},
    onOpenGeneralSettings: () -> Unit = {},
    extraBottomPadding: Dp = 0.dp,
) {
    val uriHandler = LocalUriHandler.current
    val navBarBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    var mirrorPickerVisible by rememberSaveable { mutableStateOf(false) }

    LaunchedEffect(Unit) { onShown() }

    Column(
        modifier = modifier
            .fillMaxSize()
            .springOverscroll()
            .verticalScroll(rememberScrollState())
            .padding(
                start = 24.dp,
                end = 24.dp,
                top = 20.dp,
                bottom = maxOf(navBarBottom + 16.dp, extraBottomPadding + 12.dp),
            ),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = 720.dp)
                .fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            InsetGroupedCard {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Surface(
                        shape = MaterialTheme.shapes.medium,
                        color = MaterialTheme.colorScheme.primaryContainer,
                        modifier = Modifier.size(72.dp),
                    ) {
                        Image(
                            painter = painterResource(R.drawable.ic_launcher_foreground),
                            contentDescription = null,
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(4.dp),
                        )
                    }
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
                            text = stringResource(state.build.channel.labelRes),
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                    }
                }
            }

            InsetGroupedCard {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = stringResource(R.string.about_update_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    if (state.build.channel.isUpdateSupported) {
                        UpdateStatus(
                            status = state.updateStatus,
                            downloadMirror = state.downloadMirror,
                            onCheckForUpdates = onCheckForUpdates,
                            onDownloadUpdate = onDownloadUpdate,
                            onCancelDownload = onCancelDownload,
                            onInstallUpdate = onInstallUpdate,
                            onIgnoreUpdate = onIgnoreUpdate,
                            onClearIgnoredUpdate = onClearIgnoredUpdate,
                            onSwitchMirror = { mirrorPickerVisible = true },
                            onOpenRelease = { url -> uriHandler.openUri(url) },
                        )
                    } else {
                        val channelLabel = stringResource(state.build.channel.labelRes)
                        Text(
                            text = stringResource(R.string.about_update_disabled_for_channel, channelLabel),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            InsetGroupedCard {
                TwoLineItemRow(
                    headline = stringResource(R.string.about_update_preferences),
                    supportingText = stringResource(
                        R.string.about_update_preferences_summary,
                        stringResource(state.downloadMirror.labelRes()),
                    ),
                    leadingContent = {
                        Icon(
                            Icons.Rounded.Tune,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                        )
                    },
                    onClick = onOpenGeneralSettings,
                )
                TwoLineItemDivider(startIndent = 56.dp)
                TwoLineItemRow(
                    headline = stringResource(R.string.about_project_page),
                    supportingText = PROJECT_URL,
                    leadingContent = {
                        Icon(
                            Icons.AutoMirrored.Rounded.OpenInNew,
                            contentDescription = stringResource(R.string.about_open_external),
                            tint = MaterialTheme.colorScheme.primary,
                        )
                    },
                    onClick = { uriHandler.openUri(PROJECT_URL) },
                )
            }
        }
    }

    if (state.showCellularConfirmDialog) {
        val assetSize = (state.updateStatus as? AboutUpdateStatus.UpdateAvailable)?.matchingAsset?.sizeBytes
        AlertDialog(
            onDismissRequest = onDismissCellularDownload,
            title = { Text(stringResource(R.string.about_cellular_confirm_title)) },
            text = {
                Text(
                    if (assetSize != null) {
                        stringResource(R.string.about_cellular_confirm_body_size, formatByteSize(assetSize))
                    } else {
                        stringResource(R.string.about_cellular_confirm_body)
                    },
                )
            },
            confirmButton = {
                Button(onClick = onConfirmCellularDownload) {
                    Text(stringResource(R.string.about_cellular_confirm_action))
                }
            },
            dismissButton = {
                TextButton(onClick = onDismissCellularDownload) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    if (state.showInstallPermissionDialog) {
        AlertDialog(
            onDismissRequest = onDismissInstallPermissionDialog,
            title = { Text(stringResource(R.string.about_install_permission_title)) },
            text = { Text(stringResource(R.string.about_install_permission_body)) },
            confirmButton = {
                Button(onClick = onOpenInstallPermissionSettings) {
                    Text(stringResource(R.string.about_install_permission_action))
                }
            },
            dismissButton = {
                TextButton(onClick = onDismissInstallPermissionDialog) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    if (mirrorPickerVisible) {
        val options = listOf(
            DownloadMirror.AUTO,
            DownloadMirror.DIRECT,
            DownloadMirror.GH_NET,
            DownloadMirror.GH_PROXY,
        ).map { mirror ->
            SonaOptionItem(
                value = mirror,
                label = stringResource(mirror.labelRes()),
            )
        }
        SonaSingleChoiceDialog(
            title = stringResource(R.string.general_download_mirror),
            options = options,
            selectedValue = state.downloadMirror,
            onSelect = onRetryWithMirror,
            onDismiss = { mirrorPickerVisible = false },
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun UpdateStatus(
    status: AboutUpdateStatus,
    downloadMirror: DownloadMirror,
    onCheckForUpdates: () -> Unit,
    onDownloadUpdate: () -> Unit,
    onCancelDownload: () -> Unit,
    onInstallUpdate: () -> Unit,
    onIgnoreUpdate: () -> Unit,
    onClearIgnoredUpdate: () -> Unit,
    onSwitchMirror: () -> Unit,
    onOpenRelease: (String) -> Unit,
) {
    val icon = when (status) {
        AboutUpdateStatus.Idle -> Icons.Rounded.SystemUpdate
        AboutUpdateStatus.Checking -> null
        is AboutUpdateStatus.UpToDate -> Icons.Rounded.CheckCircle
        is AboutUpdateStatus.UpdateAvailable -> Icons.Rounded.SystemUpdate
        is AboutUpdateStatus.Downloading -> Icons.Rounded.Download
        is AboutUpdateStatus.Verifying -> null
        is AboutUpdateStatus.ReadyToInstall -> Icons.Rounded.InstallMobile
        is AboutUpdateStatus.Error -> Icons.Rounded.ErrorOutline
    }

    val messageText = when (status) {
        AboutUpdateStatus.Idle -> stringResource(R.string.about_update_idle)
        AboutUpdateStatus.Checking -> stringResource(R.string.about_update_checking)
        is AboutUpdateStatus.UpToDate -> stringResource(R.string.about_update_current)
        is AboutUpdateStatus.UpdateAvailable -> stringResource(
            R.string.about_update_available,
            status.release.versionName,
        )
        is AboutUpdateStatus.Downloading -> stringResource(R.string.about_update_downloading)
        is AboutUpdateStatus.Verifying -> stringResource(R.string.about_update_verifying)
        is AboutUpdateStatus.ReadyToInstall -> stringResource(
            R.string.about_update_ready_to_install,
            status.release.versionName,
        )
        is AboutUpdateStatus.Error -> when (status.category) {
            AppUpdateErrorCategory.NETWORK -> stringResource(R.string.about_update_failed)
            AppUpdateErrorCategory.VERIFICATION_FAILED -> stringResource(R.string.about_update_verify_failed)
            AppUpdateErrorCategory.UNSUPPORTED_ABI -> stringResource(R.string.about_update_unsupported_abi)
            AppUpdateErrorCategory.WIFI_REQUIRED -> stringResource(R.string.about_update_wifi_required)
            AppUpdateErrorCategory.INSTALL_PERMISSION_REQUIRED -> stringResource(R.string.about_install_permission_title)
            AppUpdateErrorCategory.INSTALL_FAILED -> stringResource(R.string.about_update_install_failed)
        }
    }

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (status is AboutUpdateStatus.Checking || status is AboutUpdateStatus.Verifying) {
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
                text = messageText,
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.weight(1f),
            )
        }

        val release = when (status) {
            is AboutUpdateStatus.UpdateAvailable -> status.release
            is AboutUpdateStatus.ReadyToInstall -> status.release
            is AboutUpdateStatus.Error -> status.release
            else -> null
        }

        if (release != null) {
            val metadataLine = buildList {
                release.pubDate?.take(10)?.let { add(it) }
                if (status is AboutUpdateStatus.UpdateAvailable && status.matchingAsset != null) {
                    add("${status.matchingAsset.abi} · ${formatByteSize(status.matchingAsset.sizeBytes)}")
                }
                if (release.signing == "debug") {
                    add(stringResource(R.string.about_update_preview_signed))
                }
            }.joinToString(" · ")
            if (metadataLine.isNotEmpty()) {
                Text(
                    text = metadataLine,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            release.notes?.let { notes ->
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        text = notes,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(12.dp),
                    )
                }
            }
        }

        if (status is AboutUpdateStatus.Downloading) {
            val total = status.totalBytes ?: 0L
            if (total > 0L) {
                val fraction = (status.downloadedBytes.toFloat() / total.toFloat()).coerceIn(0f, 1f)
                LinearProgressIndicator(
                    progress = { fraction },
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    text = "${formatByteSize(status.downloadedBytes)} / ${formatByteSize(total)} (${(fraction * 100).toInt()}%)",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                Text(
                    text = formatByteSize(status.downloadedBytes),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        FlowRow(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            when (status) {
                is AboutUpdateStatus.UpdateAvailable -> {
                    TextButton(onClick = { onOpenRelease(status.release.releasePageUrl) }) {
                        Text(stringResource(R.string.about_view_update))
                    }
                    if (status.isIgnored) {
                        OutlinedButton(onClick = onClearIgnoredUpdate) {
                            Text(stringResource(R.string.about_update_unignore))
                        }
                    } else {
                        TextButton(onClick = onIgnoreUpdate) {
                            Text(stringResource(R.string.about_update_ignore))
                        }
                    }
                    if (status.matchingAsset != null) {
                        Button(onClick = onDownloadUpdate) {
                            Icon(Icons.Rounded.Download, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(stringResource(R.string.about_download_install))
                        }
                    } else {
                        FilledTonalButton(onClick = { onOpenRelease(status.release.releasePageUrl) }) {
                            Text(stringResource(R.string.about_view_update))
                            Spacer(Modifier.width(6.dp))
                            Icon(Icons.AutoMirrored.Rounded.OpenInNew, contentDescription = null, modifier = Modifier.size(18.dp))
                        }
                    }
                }
                is AboutUpdateStatus.Downloading -> {
                    OutlinedButton(onClick = onCancelDownload) {
                        Text(stringResource(R.string.action_cancel))
                    }
                }
                is AboutUpdateStatus.Verifying -> Unit
                is AboutUpdateStatus.ReadyToInstall -> {
                    TextButton(onClick = { onOpenRelease(status.release.releasePageUrl) }) {
                        Text(stringResource(R.string.about_view_update))
                    }
                    Button(onClick = onInstallUpdate) {
                        Icon(Icons.Rounded.InstallMobile, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(R.string.about_install_now))
                    }
                }
                is AboutUpdateStatus.Error -> {
                    status.release?.let { errRelease ->
                        TextButton(onClick = { onOpenRelease(errRelease.releasePageUrl) }) {
                            Text(stringResource(R.string.about_view_update))
                        }
                        OutlinedButton(onClick = onSwitchMirror) {
                            Text(
                                stringResource(
                                    R.string.about_switch_mirror,
                                    stringResource(downloadMirror.labelRes()),
                                ),
                            )
                        }
                        Button(onClick = onDownloadUpdate) {
                            Icon(Icons.Rounded.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(stringResource(R.string.about_retry_download))
                        }
                    } ?: run {
                        FilledTonalButton(onClick = onCheckForUpdates) {
                            Icon(Icons.Rounded.Refresh, contentDescription = null)
                            Spacer(Modifier.width(8.dp))
                            Text(stringResource(R.string.about_check_updates))
                        }
                    }
                }
                AboutUpdateStatus.Idle,
                AboutUpdateStatus.Checking,
                is AboutUpdateStatus.UpToDate -> {
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
        }
    }
}

private val AppUpdateChannel.labelRes: Int
    get() = when (this) {
        AppUpdateChannel.STABLE -> R.string.about_channel_stable
        AppUpdateChannel.NIGHTLY -> R.string.about_channel_nightly
        AppUpdateChannel.CI -> R.string.about_channel_ci
        AppUpdateChannel.DEV -> R.string.about_channel_dev
    }
