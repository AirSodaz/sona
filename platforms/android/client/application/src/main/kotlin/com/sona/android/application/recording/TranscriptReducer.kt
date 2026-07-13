package com.sona.android.application.recording

object TranscriptReducer {
    fun apply(
        current: List<TranscriptSegment>,
        update: TranscriptUpdate,
    ): List<TranscriptSegment> {
        val segmentsById = LinkedHashMap<String, TranscriptSegment>()
        current.forEach { segment -> segmentsById[segment.id] = segment }
        update.removeIds.forEach(segmentsById::remove)
        update.upsertSegments.forEach { segment -> segmentsById[segment.id] = segment }
        return segmentsById.values.sortedWith(
            compareBy<TranscriptSegment>(TranscriptSegment::startSeconds)
                .thenBy(TranscriptSegment::endSeconds)
                .thenBy(TranscriptSegment::id),
        )
    }
}
