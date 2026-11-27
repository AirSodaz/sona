package com.sona.android.app.ui.component

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.UnfoldMore
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import com.sona.android.app.R

/**
 * Option entry displayed inside [SonaSingleChoiceDialog].
 */
data class SonaOptionItem<T>(
    val value: T,
    val label: String,
    val supportingText: String? = null,
    val enabled: Boolean = true,
)

/**
 * Centered single-choice dialog following Android ListPreference / MD3 dialog conventions.
 * Selecting an item immediately applies the choice and dismisses the dialog.
 */
@Composable
fun <T> SonaSingleChoiceDialog(
    title: String,
    options: List<SonaOptionItem<T>>,
    selectedValue: T?,
    onSelect: (T) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    hasSelection: Boolean = true,
    dismissText: String = stringResource(R.string.action_cancel),
) {
    if (options.isEmpty()) {
        LaunchedEffect(Unit) { onDismiss() }
        return
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        modifier = modifier,
        title = { Text(title) },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 360.dp)
                    .verticalScroll(rememberScrollState())
                    .then(if (hasSelection) Modifier.selectableGroup() else Modifier),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                options.forEach { option ->
                    val selected = hasSelection && option.value == selectedValue
                    val onOptionClick = {
                        onSelect(option.value)
                        onDismiss()
                    }
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .defaultMinSize(minHeight = 48.dp)
                            .clip(RoundedCornerShape(12.dp))
                            .then(
                                if (hasSelection) {
                                    Modifier.selectable(
                                        selected = selected,
                                        enabled = option.enabled,
                                        role = Role.RadioButton,
                                        onClick = onOptionClick,
                                    )
                                } else {
                                    Modifier.clickable(
                                        enabled = option.enabled,
                                        role = Role.Button,
                                        onClick = onOptionClick,
                                    )
                                },
                            )
                            .padding(
                                horizontal = if (hasSelection) 4.dp else 12.dp,
                                vertical = 8.dp,
                            ),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        if (hasSelection) {
                            RadioButton(
                                selected = selected,
                                onClick = null,
                                enabled = option.enabled,
                            )
                            Spacer(modifier = Modifier.width(12.dp))
                        }
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = option.label,
                                style = MaterialTheme.typography.bodyLarge,
                                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                                color = when {
                                    !option.enabled ->
                                        MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                                    selected -> MaterialTheme.colorScheme.primary
                                    else -> MaterialTheme.colorScheme.onSurface
                                },
                            )
                            if (!option.supportingText.isNullOrBlank()) {
                                Text(
                                    text = option.supportingText,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = if (option.enabled) {
                                        MaterialTheme.colorScheme.onSurfaceVariant
                                    } else {
                                        MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.38f)
                                    },
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(dismissText)
            }
        },
    )
}

/**
 * Native Android setting row displaying a title, the currently selected value,
 * and a picker affordance that opens a [SonaSingleChoiceDialog].
 */
@Composable
fun SonaOptionSettingRow(
    headline: String,
    selectedValueText: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    leadingContent: (@Composable () -> Unit)? = null,
) {
    TwoLineItemRow(
        headline = headline,
        supportingText = selectedValueText,
        leadingContent = leadingContent,
        trailingContent = {
            Icon(
                imageVector = Icons.Rounded.UnfoldMore,
                contentDescription = null,
                tint = if (enabled) {
                    MaterialTheme.colorScheme.onSurfaceVariant
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.38f)
                },
            )
        },
        onClick = onClick,
        enabled = enabled,
        modifier = modifier,
    )
}

/**
 * Centered single-line text-input dialog with IME Done handling scoped
 * to the dialog window's own [LocalFocusManager] and [LocalSoftwareKeyboardController].
 */
@Composable
fun SonaTextInputDialog(
    title: String,
    value: String,
    onValueChange: (String) -> Unit,
    confirmText: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    label: String? = null,
    confirmEnabled: Boolean = value.isNotBlank(),
    dismissText: String = stringResource(R.string.action_cancel),
    extraContent: (@Composable ColumnScope.() -> Unit)? = null,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        modifier = modifier,
        title = { Text(title) },
        text = {
            val focusManager = LocalFocusManager.current
            val keyboardController = LocalSoftwareKeyboardController.current
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = value,
                    onValueChange = onValueChange,
                    label = label?.let { text -> { Text(text) } },
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Sentences,
                        imeAction = ImeAction.Done,
                    ),
                    keyboardActions = KeyboardActions(
                        onDone = {
                            keyboardController?.hide()
                            focusManager.clearFocus()
                            if (confirmEnabled) onConfirm()
                        },
                    ),
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                extraContent?.invoke(this)
            }
        },
        confirmButton = {
            TextButton(
                enabled = confirmEnabled,
                onClick = onConfirm,
            ) {
                Text(confirmText)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(dismissText)
            }
        },
    )
}
