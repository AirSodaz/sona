package com.sona.android.app.feature.library

import androidx.annotation.StringRes
import com.sona.android.app.R
import com.sona.android.application.library.HistoryDateFilter
import com.sona.android.application.library.HistoryFilterType
import com.sona.android.application.library.HistorySortOrder
import com.sona.android.application.library.TranscriptSnapshotReason
import com.sona.android.application.recording.AudioImportFailure

@StringRes
fun HistoryFilterType.labelRes(): Int = when (this) {
    HistoryFilterType.ALL -> R.string.history_filter_all
    HistoryFilterType.RECORDING -> R.string.history_filter_recording
    HistoryFilterType.BATCH -> R.string.history_filter_batch
}

@StringRes
fun HistoryDateFilter.labelRes(): Int = when (this) {
    HistoryDateFilter.ALL -> R.string.history_date_all
    HistoryDateFilter.TODAY -> R.string.history_date_today
    HistoryDateFilter.WEEK -> R.string.history_date_week
    HistoryDateFilter.MONTH -> R.string.history_date_month
}

@StringRes
fun HistorySortOrder.labelRes(): Int = when (this) {
    HistorySortOrder.NEWEST -> R.string.history_sort_newest
    HistorySortOrder.OLDEST -> R.string.history_sort_oldest
    HistorySortOrder.DURATION_DESC -> R.string.history_sort_duration_desc
    HistorySortOrder.DURATION_ASC -> R.string.history_sort_duration_asc
    HistorySortOrder.TITLE_ASC -> R.string.history_sort_title_asc
}

@StringRes
fun TranscriptSnapshotReason.labelRes(): Int = when (this) {
    TranscriptSnapshotReason.POLISH -> R.string.snapshot_reason_polish
    TranscriptSnapshotReason.TRANSLATE -> R.string.snapshot_reason_translate
    TranscriptSnapshotReason.RETRANSCRIBE -> R.string.snapshot_reason_retranscribe
    TranscriptSnapshotReason.RESTORE -> R.string.snapshot_reason_restore
    TranscriptSnapshotReason.MANUAL_EDIT -> R.string.snapshot_reason_manual_edit
}

@StringRes
fun AudioImportFailure.messageRes(): Int = when (this) {
    AudioImportFailure.INVALID_SOURCE -> R.string.audio_import_error_invalid_source
    AudioImportFailure.UNSUPPORTED_AUDIO -> R.string.audio_import_error_unsupported_audio
    AudioImportFailure.DURATION_LIMIT -> R.string.audio_import_error_duration_limit
    AudioImportFailure.STORAGE -> R.string.audio_import_error_storage
    AudioImportFailure.CONFIGURATION -> R.string.audio_import_error_configuration
    AudioImportFailure.TRANSCODING -> R.string.audio_import_error_transcoding
    AudioImportFailure.TRANSCRIPTION -> R.string.audio_import_error_transcription
    AudioImportFailure.PERSISTENCE -> R.string.audio_import_error_persistence
}
