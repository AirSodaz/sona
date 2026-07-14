package com.sona.android.app.feature.settings

import androidx.annotation.StringRes
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.ui.graphics.vector.ImageVector
import com.sona.android.app.R

internal enum class SettingsSection(
    val route: String,
    @param:StringRes val labelRes: Int,
    @param:StringRes val summaryRes: Int,
    val icon: ImageVector,
) {
    APPEARANCE(
        route = "appearance",
        labelRes = R.string.appearance_heading,
        summaryRes = R.string.settings_appearance_summary,
        icon = Icons.Rounded.Palette,
    ),
    RECOGNITION(
        route = "recognition",
        labelRes = R.string.settings_recognition_heading,
        summaryRes = R.string.settings_recognition_summary,
        icon = Icons.Rounded.GraphicEq,
    ),
    ;

    companion object {
        fun fromRoute(route: String?): SettingsSection? =
            entries.firstOrNull { it.route == route }
    }
}
