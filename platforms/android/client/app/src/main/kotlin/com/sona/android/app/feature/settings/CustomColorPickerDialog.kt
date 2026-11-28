package com.sona.android.app.feature.settings

import android.graphics.Color as AndroidColor
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.ui.theme.SonaColorEngine
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import java.util.Locale

private val QUICK_SEED_COLORS = listOf(
    0xFF7B4E3B.toInt(),
    0xFFD94A38.toInt(),
    0xFFCC7A00.toInt(),
    0xFF2E7D32.toInt(),
    0xFF00796B.toInt(),
    0xFF1976D2.toInt(),
    0xFF5E35B1.toInt(),
    0xFFC2185B.toInt(),
)

@Composable
internal fun CustomColorPickerDialog(
    initialSeedArgb: Int,
    paletteStyle: PaletteStyle,
    contrastLevel: ContrastLevel,
    pureBlackDarkEnabled: Boolean = false,
    isDark: Boolean = isSystemInDarkTheme(),
    onApply: (Int) -> Unit,
    onDismiss: () -> Unit,
) {
    val initialHsv = remember(initialSeedArgb) {
        FloatArray(3).apply { AndroidColor.colorToHSV(initialSeedArgb or 0xFF000000.toInt(), this) }
    }
    var selectedArgb by rememberSaveable(initialSeedArgb) {
        mutableIntStateOf(initialSeedArgb or 0xFF000000.toInt())
    }
    var hue by rememberSaveable(initialSeedArgb) { mutableFloatStateOf(initialHsv[0]) }
    var saturation by rememberSaveable(initialSeedArgb) { mutableFloatStateOf(initialHsv[1]) }
    var value by rememberSaveable(initialSeedArgb) { mutableFloatStateOf(initialHsv[2]) }
    var hexInput by rememberSaveable(initialSeedArgb) {
        mutableStateOf(formatHex(initialSeedArgb))
    }
    var showValidationOnIncomplete by rememberSaveable(initialSeedArgb) { mutableStateOf(false) }

    val parsedHex = remember(hexInput) {
        if (hexInput.length == 6) hexInput.toIntOrNull(16) else null
    }
    val isHexValid = parsedHex != null
    val showHexError = (hexInput.length == 6 && !isHexValid) ||
        (showValidationOnIncomplete && !isHexValid)

    val currentArgb = selectedArgb
    val currentColor = Color(currentArgb)
    val generatedPreviewScheme = remember(currentArgb, paletteStyle, contrastLevel, pureBlackDarkEnabled, isDark) {
        val raw = SonaColorEngine.generateColorScheme(
            seedArgb = currentArgb,
            style = paletteStyle,
            contrastLevel = contrastLevel,
            isDark = isDark,
        )
        if (isDark && pureBlackDarkEnabled) {
            SonaColorEngine.applyPureBlackSurfaces(raw, contrastLevel)
        } else {
            raw
        }
    }

    val syncFromArgb: (Int) -> Unit = { argb ->
        val opaque = argb or 0xFF000000.toInt()
        val hsv = FloatArray(3)
        AndroidColor.colorToHSV(opaque, hsv)
        selectedArgb = opaque
        hue = hsv[0]
        saturation = hsv[1]
        value = hsv[2]
        hexInput = formatHex(opaque)
        showValidationOnIncomplete = false
    }

    val focusManager = LocalFocusManager.current
    val keyboardController = LocalSoftwareKeyboardController.current
    val hueLabel = stringResource(R.string.custom_color_hue)

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.custom_color_dialog_title)) },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 480.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(16.dp),
                    color = generatedPreviewScheme.surfaceContainer,
                    border = BorderStroke(1.dp, generatedPreviewScheme.outlineVariant),
                ) {
                    Row(
                        modifier = Modifier.padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(44.dp)
                                .clip(CircleShape)
                                .background(currentColor)
                                .border(1.5.dp, generatedPreviewScheme.outlineVariant, CircleShape),
                        )
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = "#${formatHex(currentArgb)}",
                                style = MaterialTheme.typography.titleMedium,
                                fontFamily = FontFamily.Monospace,
                                fontWeight = FontWeight.SemiBold,
                                color = generatedPreviewScheme.onSurface,
                            )
                            Spacer(modifier = Modifier.height(6.dp))
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .height(14.dp)
                                        .clip(RoundedCornerShape(999.dp))
                                        .background(generatedPreviewScheme.primary),
                                )
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .height(14.dp)
                                        .clip(RoundedCornerShape(999.dp))
                                        .background(generatedPreviewScheme.primaryContainer),
                                )
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .height(14.dp)
                                        .clip(RoundedCornerShape(999.dp))
                                        .background(generatedPreviewScheme.tertiary),
                                )
                            }
                        }
                    }
                }

                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .selectableGroup(),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    QUICK_SEED_COLORS.chunked(4).forEach { rowColors ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceEvenly,
                        ) {
                            rowColors.forEach { presetArgb ->
                                val isSelected = (presetArgb and 0x00FFFFFF) == (currentArgb and 0x00FFFFFF)
                                val swatchDescription = stringResource(
                                    R.string.custom_color_swatch_description,
                                    formatHex(presetArgb),
                                )
                                Box(
                                    modifier = Modifier
                                        .size(48.dp)
                                        .clip(CircleShape)
                                        .selectable(
                                            selected = isSelected,
                                            role = Role.RadioButton,
                                            onClick = { syncFromArgb(presetArgb) },
                                        )
                                        .semantics { contentDescription = swatchDescription },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .size(28.dp)
                                            .clip(CircleShape)
                                            .background(Color(presetArgb))
                                            .border(
                                                width = if (isSelected) 2.5.dp else 1.dp,
                                                color = if (isSelected) {
                                                    MaterialTheme.colorScheme.onSurface
                                                } else {
                                                    MaterialTheme.colorScheme.outlineVariant
                                                },
                                                shape = CircleShape,
                                            ),
                                    )
                                }
                            }
                        }
                    }
                }

                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(
                            text = hueLabel,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            text = "${hue.toInt()}°",
                            style = MaterialTheme.typography.labelMedium,
                            fontFamily = FontFamily.Monospace,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                    }
                    Box(
                        modifier = Modifier.fillMaxWidth(),
                        contentAlignment = Alignment.Center,
                    ) {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(10.dp)
                                .clip(RoundedCornerShape(999.dp))
                                .background(
                                    Brush.horizontalGradient(
                                        colors = listOf(
                                            Color.Red,
                                            Color.Yellow,
                                            Color.Green,
                                            Color.Cyan,
                                            Color.Blue,
                                            Color.Magenta,
                                            Color.Red,
                                        ),
                                    ),
                                ),
                        )
                        Slider(
                            value = hue,
                            onValueChange = { nextHue ->
                                hue = nextHue
                                val nextArgb = AndroidColor.HSVToColor(floatArrayOf(nextHue, saturation, value)) or 0xFF000000.toInt()
                                selectedArgb = nextArgb
                                hexInput = formatHex(nextArgb)
                                showValidationOnIncomplete = false
                            },
                            valueRange = 0f..360f,
                            colors = SliderDefaults.colors(
                                thumbColor = Color.hsv(hue, 1f, 1f),
                                activeTrackColor = Color.Transparent,
                                inactiveTrackColor = Color.Transparent,
                            ),
                            modifier = Modifier
                                .fillMaxWidth()
                                .semantics { contentDescription = hueLabel },
                        )
                    }
                }

                LabeledChannelSlider(
                    label = stringResource(R.string.custom_color_saturation),
                    valueText = "${(saturation * 100).toInt()}%",
                    value = saturation,
                    activeColor = currentColor,
                    onValueChange = { nextSat ->
                        saturation = nextSat
                        val nextArgb = AndroidColor.HSVToColor(floatArrayOf(hue, nextSat, value)) or 0xFF000000.toInt()
                        selectedArgb = nextArgb
                        hexInput = formatHex(nextArgb)
                        showValidationOnIncomplete = false
                    },
                )

                LabeledChannelSlider(
                    label = stringResource(R.string.custom_color_brightness),
                    valueText = "${(value * 100).toInt()}%",
                    value = value,
                    activeColor = currentColor,
                    onValueChange = { nextVal ->
                        value = nextVal
                        val nextArgb = AndroidColor.HSVToColor(floatArrayOf(hue, saturation, nextVal)) or 0xFF000000.toInt()
                        selectedArgb = nextArgb
                        hexInput = formatHex(nextArgb)
                        showValidationOnIncomplete = false
                    },
                )

                OutlinedTextField(
                    value = hexInput,
                    onValueChange = { raw ->
                        val filtered = raw.removePrefix("#")
                            .uppercase(Locale.ROOT)
                            .filter { it in '0'..'9' || it in 'A'..'F' }
                            .take(6)
                        hexInput = filtered
                        if (filtered.length == 6) {
                            val parsed = filtered.toIntOrNull(16)
                            if (parsed != null) {
                                val argb = parsed or 0xFF000000.toInt()
                                val hsv = FloatArray(3)
                                AndroidColor.colorToHSV(argb, hsv)
                                selectedArgb = argb
                                hue = hsv[0]
                                saturation = hsv[1]
                                value = hsv[2]
                                showValidationOnIncomplete = false
                            }
                        }
                    },
                    label = { Text(stringResource(R.string.custom_color_hex_label)) },
                    prefix = { Text("#", fontFamily = FontFamily.Monospace) },
                    isError = showHexError,
                    supportingText = if (showHexError) {
                        { Text(stringResource(R.string.custom_color_hex_invalid)) }
                    } else {
                        null
                    },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Characters,
                        imeAction = ImeAction.Done,
                    ),
                    keyboardActions = KeyboardActions(
                        onDone = {
                            keyboardController?.hide()
                            if (isHexValid) {
                                focusManager.clearFocus()
                                onApply(currentColor.toArgb())
                                onDismiss()
                            } else {
                                showValidationOnIncomplete = true
                            }
                        },
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .onFocusChanged { focusState ->
                            if (!focusState.isFocused && !isHexValid) {
                                showValidationOnIncomplete = true
                            }
                        },
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = isHexValid,
                onClick = {
                    keyboardController?.hide()
                    onApply(currentColor.toArgb())
                    onDismiss()
                },
            ) {
                Text(stringResource(R.string.custom_color_apply))
            }
        },
        dismissButton = {
            TextButton(
                onClick = {
                    keyboardController?.hide()
                    onDismiss()
                },
            ) {
                Text(stringResource(R.string.action_cancel))
            }
        },
    )
}

@Composable
private fun LabeledChannelSlider(
    label: String,
    valueText: String,
    value: Float,
    activeColor: Color,
    onValueChange: (Float) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(0.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = valueText,
                style = MaterialTheme.typography.labelMedium,
                fontFamily = FontFamily.Monospace,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
        Slider(
            value = value,
            onValueChange = onValueChange,
            valueRange = 0f..1f,
            colors = SliderDefaults.colors(
                thumbColor = activeColor,
                activeTrackColor = activeColor,
            ),
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = label },
        )
    }
}

private fun formatHex(argb: Int): String =
    String.format(Locale.ROOT, "%06X", argb and 0x00FFFFFF)
