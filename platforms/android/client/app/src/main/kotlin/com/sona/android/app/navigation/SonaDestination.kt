package com.sona.android.app.navigation

import androidx.annotation.StringRes
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.ui.graphics.vector.ImageVector
import com.sona.android.app.R

enum class SonaDestination(
    val route: String,
    @param:StringRes val labelRes: Int,
    val icon: ImageVector,
) {
    RECORD("record", R.string.destination_record, Icons.Rounded.Mic),
    LIBRARY("library", R.string.destination_library, Icons.Rounded.FolderOpen),
    SETTINGS("settings", R.string.destination_settings, Icons.Rounded.Settings),
}
