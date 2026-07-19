package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.SpeakerAttribution
import com.sona.android.application.recording.SpeakerCandidate
import com.sona.android.application.recording.SpeakerTag
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.TranscriptTiming
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import com.sona.android.application.recording.TranscriptTimingUnit
import com.sona.android.application.recording.TranscriptUpdate
import uniffi.sona_uniffi_bind.FfiSpeakerAttribution
import uniffi.sona_uniffi_bind.FfiSpeakerCandidate
import uniffi.sona_uniffi_bind.FfiSpeakerTag
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import uniffi.sona_uniffi_bind.FfiTranscriptTiming
import uniffi.sona_uniffi_bind.FfiTranscriptTimingLevel
import uniffi.sona_uniffi_bind.FfiTranscriptTimingSource
import uniffi.sona_uniffi_bind.FfiTranscriptTimingUnit
import uniffi.sona_uniffi_bind.FfiTranscriptUpdate

internal fun FfiTranscriptUpdate.toApplication(): TranscriptUpdate = TranscriptUpdate(
    removeIds = removeIds,
    upsertSegments = upsertSegments.map(FfiTranscriptSegment::toApplication),
)

internal fun TranscriptSegment.toFfi(): FfiTranscriptSegment = FfiTranscriptSegment(
    id = id,
    text = text,
    start = startSeconds,
    end = endSeconds,
    isFinal = isFinal,
    timing = timing?.toFfi(),
    tokens = tokens,
    timestamps = timestamps,
    durations = durations,
    translation = translation,
    speaker = speaker?.toFfi(),
    speakerAttribution = speakerAttribution?.toFfi(),
)

internal fun FfiTranscriptSegment.toApplication(): TranscriptSegment = TranscriptSegment(
    id = id,
    text = text,
    startSeconds = start,
    endSeconds = end,
    isFinal = isFinal,
    timing = timing?.toApplication(),
    tokens = tokens,
    timestamps = timestamps,
    durations = durations,
    translation = translation,
    speaker = speaker?.toApplication(),
    speakerAttribution = speakerAttribution?.toApplication(),
)

private fun FfiTranscriptTiming.toApplication(): TranscriptTiming = TranscriptTiming(
    level = when (level) {
        FfiTranscriptTimingLevel.TOKEN -> TranscriptTimingLevel.TOKEN
        FfiTranscriptTimingLevel.SEGMENT -> TranscriptTimingLevel.SEGMENT
    },
    source = when (source) {
        FfiTranscriptTimingSource.MODEL -> TranscriptTimingSource.MODEL
        FfiTranscriptTimingSource.DERIVED -> TranscriptTimingSource.DERIVED
    },
    units = units.map(FfiTranscriptTimingUnit::toApplication),
)

private fun TranscriptTiming.toFfi(): FfiTranscriptTiming = FfiTranscriptTiming(
    level = when (level) {
        TranscriptTimingLevel.TOKEN -> FfiTranscriptTimingLevel.TOKEN
        TranscriptTimingLevel.SEGMENT -> FfiTranscriptTimingLevel.SEGMENT
    },
    source = when (source) {
        TranscriptTimingSource.MODEL -> FfiTranscriptTimingSource.MODEL
        TranscriptTimingSource.DERIVED -> FfiTranscriptTimingSource.DERIVED
    },
    units = units.map(TranscriptTimingUnit::toFfi),
)

private fun FfiTranscriptTimingUnit.toApplication(): TranscriptTimingUnit = TranscriptTimingUnit(
    text = text,
    startSeconds = start,
    endSeconds = end,
)

private fun TranscriptTimingUnit.toFfi(): FfiTranscriptTimingUnit = FfiTranscriptTimingUnit(
    text = text,
    start = startSeconds,
    end = endSeconds,
)

private fun FfiSpeakerTag.toApplication(): SpeakerTag = SpeakerTag(
    id = id,
    label = label,
    kind = kind,
    score = score,
)

private fun SpeakerTag.toFfi(): FfiSpeakerTag = FfiSpeakerTag(
    id = id,
    label = label,
    kind = kind,
    score = score,
)

private fun FfiSpeakerCandidate.toApplication(): SpeakerCandidate = SpeakerCandidate(
    profileId = profileId,
    profileName = profileName,
    score = score,
    rank = rank,
)

private fun SpeakerCandidate.toFfi(): FfiSpeakerCandidate = FfiSpeakerCandidate(
    profileId = profileId,
    profileName = profileName,
    score = score,
    rank = rank,
)

private fun FfiSpeakerAttribution.toApplication(): SpeakerAttribution = SpeakerAttribution(
    groupId = groupId,
    anonymousLabel = anonymousLabel,
    state = state,
    source = source,
    confidence = confidence,
    candidates = candidates.map(FfiSpeakerCandidate::toApplication),
)

private fun SpeakerAttribution.toFfi(): FfiSpeakerAttribution = FfiSpeakerAttribution(
    groupId = groupId,
    anonymousLabel = anonymousLabel,
    state = state,
    source = source,
    confidence = confidence,
    candidates = candidates.map(SpeakerCandidate::toFfi),
)
