package com.sona.android.application.recording

import org.junit.Assert.assertEquals
import org.junit.Test

class TranscriptReducerTest {
    @Test
    fun `removes segments before replacing and inserting updates`() {
        val current = listOf(
            segment(id = "old", start = 0.0, text = "old"),
            segment(id = "replace", start = 2.0, text = "before"),
        )
        val update = TranscriptUpdate(
            removeIds = listOf("old", "replace"),
            upsertSegments = listOf(
                segment(id = "replace", start = 1.0, text = "after"),
                segment(id = "new", start = 3.0, text = "new"),
            ),
        )

        assertEquals(
            listOf("replace:after", "new:new"),
            TranscriptReducer.apply(current, update).map { "${it.id}:${it.text}" },
        )
    }

    @Test
    fun `orders equal-time segments deterministically without mutating input`() {
        val current = listOf(segment(id = "z", start = 1.0, text = "z", end = 2.0))
        val result = TranscriptReducer.apply(
            current,
            TranscriptUpdate(
                removeIds = emptyList(),
                upsertSegments = listOf(
                    segment(id = "b", start = 1.0, text = "b", end = 2.0),
                    segment(id = "a", start = 1.0, text = "a", end = 2.0),
                ),
            ),
        )

        assertEquals(listOf("a", "b", "z"), result.map(TranscriptSegment::id))
        assertEquals(listOf("z"), current.map(TranscriptSegment::id))
    }

    private fun segment(
        id: String,
        start: Double,
        text: String,
        end: Double = start + 1.0,
    ): TranscriptSegment = TranscriptSegment(
        id = id,
        text = text,
        startSeconds = start,
        endSeconds = end,
        isFinal = true,
    )
}
