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
    val routePattern: String = route,
) {
    RECORD("record", R.string.destination_record, Icons.Rounded.Mic),
    LIBRARY("library", R.string.destination_library, Icons.Rounded.FolderOpen),
    SETTINGS(
        route = "settings",
        labelRes = R.string.destination_settings,
        icon = Icons.Rounded.Settings,
        routePattern = "settings?section={section}",
    ),
    ;

    fun matches(candidateRoute: String?): Boolean =
        candidateRoute?.substringBefore('?')?.let { candidate ->
            candidate == route || candidate.startsWith("$route/")
        } == true
}

internal const val SETTINGS_SECTION_ARGUMENT = "section"
