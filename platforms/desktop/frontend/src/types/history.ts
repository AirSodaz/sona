export type {
    HistoryAudioCleanupReport,
    HistoryAudioStatus,
    HistoryDraftSource,
    HistoryItemStatus,
} from '../bindings';

import type { HistoryAudioStatus, HistoryDraftSource, HistoryItemStatus } from '../bindings';
import type { HistoryItemRecord } from '../bindings';

export interface HistoryItem {
    id: string;
    timestamp: number;
    duration: number;
    audioPath: string; // Relative to app data dir or absolute
    audioStatus?: HistoryAudioStatus;
    transcriptPath: string; // Relative to app data dir or absolute
    title: string;
    previewText: string;
    icon?: string;
    type?: 'recording' | 'batch';
    searchContent?: string;
    /** @deprecated Legacy backup field; runtime ownership is projectId. */
    tagIds?: string[];
    deletedAt?: number | null;
    projectId?: string | null;
    status?: HistoryItemStatus;
    draftSource?: HistoryDraftSource;
}

export function normalizeHistoryItemRecord(
    item: Partial<HistoryItemRecord> | null | undefined,
): HistoryItem {
    return {
        id: item?.id || '',
        timestamp: item?.timestamp || 0,
        duration: item?.duration || 0,
        audioPath: item?.audioPath || '',
        audioStatus: ['available', 'missing', 'removed'].includes(item?.audioStatus || '')
            ? item?.audioStatus
            : 'available',
        transcriptPath: item?.transcriptPath || '',
        title: item?.title || '',
        previewText: item?.previewText || '',
        icon: typeof item?.icon === 'string' ? item.icon : undefined,
        type: item?.type === 'batch' ? 'batch' : 'recording',
        searchContent: item?.searchContent || '',
        projectId: typeof item?.projectId === 'string'
            ? item.projectId
            : (Array.isArray(item?.tagIds) && typeof item.tagIds[0] === 'string' ? item.tagIds[0] : null),
        tagIds: Array.isArray(item?.tagIds)
            ? item.tagIds.filter((tagId): tagId is string => typeof tagId === 'string')
            : [],
        deletedAt: typeof item?.deletedAt === 'number' ? item.deletedAt : null,
        status: item?.status === 'draft' ? 'draft' : 'complete',
        draftSource: item?.draftSource === 'live_record' ? 'live_record' : undefined,
    };
}

export function getHistoryItemStatus(item: Pick<HistoryItem, 'status'>): HistoryItemStatus {
    return item.status === 'draft' ? 'draft' : 'complete';
}

export function isHistoryItemDraft(item: Pick<HistoryItem, 'status'>): boolean {
    return getHistoryItemStatus(item) === 'draft';
}

export function isLiveRecordDraftHistoryItem(
    item: Pick<HistoryItem, 'status' | 'draftSource' | 'type'>,
): boolean {
    return item.type !== 'batch' && isHistoryItemDraft(item) && item.draftSource === 'live_record';
}
