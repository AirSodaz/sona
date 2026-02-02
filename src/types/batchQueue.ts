import { TranscriptSegment } from './transcript';

/**
 * Status of a batch queue item.
 */
export type BatchQueueItemStatus = 'pending' | 'processing' | 'complete' | 'error';

/**
 * Represents a file in the batch transcription queue.
 */
export interface BatchQueueItem {
    /** Unique identifier for the queue item. */
    id: string;
    /** Original filename (display name). */
    filename: string;
    /** Full file path for processing. */
    filePath: string;
    /** Current processing status. */
    status: BatchQueueItemStatus;
    /** Processing progress (0-100). */
    progress: number;
    /** Transcription result segments. */
    segments: TranscriptSegment[];
    /** Error message if status is 'error'. */
    errorMessage?: string;
    /** Asset URL for audio playback. */
    audioUrl?: string;
}
