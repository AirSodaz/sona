import type { AppConfig } from './config';
import type {
    AutomationExportConfig,
    AutomationResolutionSnapshot,
    AutomationStageConfig,
} from './automation';
import type { TranscriptSegment } from './transcript';
import type {
    RecoveredQueueItem_Serialize as CoreRecoveredQueueItem,
    RecoverySnapshot_Serialize as CoreRecoverySnapshot,
} from '../bindings';

export type { RecoveryItemStage, RecoveryResolution, RecoverySource } from '../bindings';

export interface RecoveredQueueItem extends Omit<
    CoreRecoveredQueueItem,
    | 'segments'
    | 'historyId'
    | 'historyTitle'
    | 'automationRuleId'
    | 'automationRuleName'
    | 'resolvedConfigSnapshot'
    | 'automationResolutionSnapshot'
    | 'exportConfig'
    | 'stageConfig'
    | 'sourceFingerprint'
    | 'fileStat'
    | 'exportFileNamePrefix'
    | 'tagIds'
> {
    segments: TranscriptSegment[];
    tagIds?: string[];
    /** @deprecated Single-tag compatibility alias. */
    projectId?: string | null;
    historyId?: string;
    historyTitle?: string;
    automationRuleId?: string;
    automationRuleName?: string;
    resolvedConfigSnapshot?: AppConfig;
    automationResolutionSnapshot?: AutomationResolutionSnapshot;
    exportConfig?: AutomationExportConfig | null;
    stageConfig?: AutomationStageConfig | null;
    sourceFingerprint?: string;
    fileStat?: {
        size: number;
        mtimeMs: number;
    };
    exportFileNamePrefix?: string;
}

export interface RecoverySnapshot extends Omit<CoreRecoverySnapshot, 'items'> {
    items: RecoveredQueueItem[];
}
