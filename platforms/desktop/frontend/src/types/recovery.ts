import type { AppConfig } from './config';
import type { AutomationExportConfig, AutomationStageConfig } from './automation';
import type { TranscriptSegment } from './transcript';

export type RecoveryItemStage =
    | 'queued'
    | 'transcribing'
    | 'polishing'
    | 'translating'
    | 'exporting';

export type RecoverySource = 'batch_import' | 'automation';

export type RecoveryResolution = 'pending' | 'resumed' | 'discarded';

export interface RecoveredQueueItem {
    id: string;
    filename: string;
    filePath: string;
    source: RecoverySource;
    resolution: RecoveryResolution;
    progress: number;
    segments: TranscriptSegment[];
    projectId: string | null;
    historyId?: string;
    historyTitle?: string;
    lastKnownStage: RecoveryItemStage;
    updatedAt: number;
    hasSourceFile: boolean;
    canResume: boolean;
    automationRuleId?: string;
    automationRuleName?: string;
    resolvedConfigSnapshot?: AppConfig;
    exportConfig?: AutomationExportConfig | null;
    stageConfig?: AutomationStageConfig | null;
    sourceFingerprint?: string;
    fileStat?: {
        size: number;
        mtimeMs: number;
    };
    exportFileNamePrefix?: string;
}

export interface RecoverySnapshot {
    version: number;
    updatedAt: number | null;
    items: RecoveredQueueItem[];
}
