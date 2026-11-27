package com.sona.android.app.feature.library

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.AutoFixHigh
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Translate
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.application.llm.LlmFailureCategory
import com.sona.android.application.llm.LlmTaskState

/**
 * First-class AI workspace tab inside `LibraryDetailScreen` for generating and
 * reading summaries, choosing translation target languages inline, and polishing transcripts.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun AiWorkspacePane(
    llm: LibraryLlmUiState,
    editorDirty: Boolean,
    appLanguage: AppLanguage,
    onSummarize: () -> Unit,
    onTranslate: (String, String?) -> Unit,
    onPolish: () -> Unit,
    onRetryLlm: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var translatePickerExpanded by remember { mutableStateOf(false) }
    var targetLanguage by remember(appLanguage) {
        mutableStateOf(appLanguage.takeUnless { it == AppLanguage.SYSTEM } ?: AppLanguage.ENGLISH)
    }
    val running = llm.task is LlmTaskState.Running
    val actionsEnabled = !editorDirty && !running

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        // Action Pills
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val buttonPadding = PaddingValues(horizontal = 10.dp, vertical = 10.dp)
            FilledTonalButton(
                onClick = onSummarize,
                enabled = actionsEnabled,
                shape = RoundedCornerShape(18.dp),
                contentPadding = buttonPadding,
                modifier = Modifier.weight(1f),
            ) {
                Icon(Icons.Rounded.AutoAwesome, null, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(4.dp))
                Text(
                    text = stringResource(R.string.llm_summarize),
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            FilledTonalButton(
                onClick = { translatePickerExpanded = !translatePickerExpanded },
                enabled = actionsEnabled,
                shape = RoundedCornerShape(18.dp),
                contentPadding = buttonPadding,
                modifier = Modifier.weight(1f),
            ) {
                Icon(Icons.Rounded.Translate, null, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(4.dp))
                Text(
                    text = stringResource(R.string.llm_translate),
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            FilledTonalButton(
                onClick = onPolish,
                enabled = actionsEnabled,
                shape = RoundedCornerShape(18.dp),
                contentPadding = buttonPadding,
                modifier = Modifier.weight(1f),
            ) {
                Icon(Icons.Rounded.AutoFixHigh, null, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(4.dp))
                Text(
                    text = stringResource(R.string.llm_polish),
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }

        // Inline Translation Target Language Picker
        AnimatedVisibility(visible = translatePickerExpanded) {
            Surface(
                shape = RoundedCornerShape(20.dp),
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = stringResource(R.string.llm_translate_title),
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        AppLanguage.entries.filter { it != AppLanguage.SYSTEM }.forEach { language ->
                            val languageName = stringResource(language.labelRes)
                            FilterChip(
                                selected = targetLanguage == language,
                                onClick = {
                                    targetLanguage = language
                                    translatePickerExpanded = false
                                    onTranslate(language.languageTag, languageName)
                                },
                                label = { Text(languageName) },
                            )
                        }
                    }
                }
            }
        }

        when (val task = llm.task) {
            is LlmTaskState.Running -> {
                Surface(
                    shape = RoundedCornerShape(20.dp),
                    color = MaterialTheme.colorScheme.secondaryContainer,
                    contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = stringResource(R.string.llm_progress, task.progress.percent),
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.Medium,
                        )
                        LinearProgressIndicator(
                            progress = { (task.progress.percent / 100f).coerceIn(0f, 1f) },
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }
                }
            }
            is LlmTaskState.Failed -> {
                Surface(
                    shape = RoundedCornerShape(20.dp),
                    color = MaterialTheme.colorScheme.errorContainer,
                    contentColor = MaterialTheme.colorScheme.onErrorContainer,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(16.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Text(
                            text = stringResource(task.category.toStringResource()),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.weight(1f),
                        )
                        FilledTonalButton(onClick = onRetryLlm) {
                            Text(stringResource(R.string.llm_retry))
                        }
                    }
                }
            }
            else -> Unit
        }

        val summary = llm.summary?.content?.takeIf(String::isNotBlank)
        if (summary != null) {
            Surface(
                shape = RoundedCornerShape(22.dp),
                color = MaterialTheme.colorScheme.surfaceContainer,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(
                    modifier = Modifier.padding(18.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.AutoAwesome,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.primary,
                                modifier = Modifier.size(18.dp),
                            )
                            Text(
                                text = stringResource(R.string.llm_summarize),
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.primary,
                            )
                        }
                        val context = LocalContext.current
                        val clipboardManager = remember(context) {
                            context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                        }
                        IconButton(
                            onClick = {
                                clipboardManager?.setPrimaryClip(
                                    ClipData.newPlainText("Sona Summary", summary),
                                )
                            },
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.ContentCopy,
                                contentDescription = stringResource(R.string.action_copy),
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                    Text(
                        text = summary,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
            }
        } else if (llm.task !is LlmTaskState.Running) {
            Surface(
                shape = RoundedCornerShape(22.dp),
                color = MaterialTheme.colorScheme.surfaceContainerLow,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(
                    modifier = Modifier.padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(
                        imageVector = Icons.Rounded.AutoAwesome,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary.copy(alpha = 0.65f),
                        modifier = Modifier.size(32.dp),
                    )
                    Text(
                        text = stringResource(R.string.ai_workspace_empty_title),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        text = stringResource(R.string.ai_workspace_empty_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        Spacer(Modifier.height(16.dp))
    }
}

private fun LlmFailureCategory.toStringResource(): Int = when (this) {
    LlmFailureCategory.NOT_CONFIGURED -> R.string.llm_not_configured
    LlmFailureCategory.AUTHENTICATION -> R.string.llm_error_authentication
    LlmFailureCategory.NETWORK -> R.string.llm_error_network
    LlmFailureCategory.RATE_LIMITED -> R.string.llm_error_rate_limited
    LlmFailureCategory.INVALID_RESPONSE -> R.string.llm_error_invalid_response
    LlmFailureCategory.UNSUPPORTED -> R.string.llm_error_unsupported
    LlmFailureCategory.UNKNOWN -> R.string.llm_error_unknown
}
