import type { AppConfig } from './config';
import { TranscriptSegment } from './transcript';
import type { AutomationExportConfig, AutomationStageConfig } from './automation';

/**
 * Status of a batch queue item.
 */
export type BatchQueueItemStatus = 'pending' | 'processing' | 'complete' | 'error';

export type BatchQueueItemOrigin = 'manual' | 'automation';

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
    /** ID of the saved history item for this queue item. */
    historyId?: string;
    /** Project context captured when this queue item was created. */
    projectId: string | null;
    /** How this queue item entered the pipeline. */
    origin?: BatchQueueItemOrigin;
    /** Automation rule ID when the item originated from folder monitoring. */
    automationRuleId?: string;
    /** Snapshot of the automation rule name for UI display. */
    automationRuleName?: string;
    /** Runtime config snapshot used when this item was queued. */
    resolvedConfigSnapshot?: AppConfig;
    /** Optional export settings used after processing. */
    exportConfig?: AutomationExportConfig | null;
    /** Optional automation stage settings captured at queue time. */
    stageConfig?: AutomationStageConfig | null;
    /** Persistent fingerprint for automation dedupe. */
    sourceFingerprint?: string;
    /** Original source file stat snapshot used for automation manifest writes. */
    fileStat?: {
        size: number;
        mtimeMs: number;
    };
    /** Exported output path when automation export succeeds. */
    exportPath?: string;
    /** Snapshot of the project export filename prefix used for automation exports. */
    exportFileNamePrefix?: string;
}
