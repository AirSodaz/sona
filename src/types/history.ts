export type HistoryItemStatus = 'draft' | 'complete';
export type HistoryDraftSource = 'live_record';

export interface HistoryItem {
    id: string;
    timestamp: number;
    duration: number;
    audioPath: string; // Relative to app data dir or absolute
    transcriptPath: string; // Relative to app data dir or absolute
    title: string;
    previewText: string;
    icon?: string;
    type?: 'recording' | 'batch';
    searchContent?: string;
    projectId: string | null;
    status?: HistoryItemStatus;
    draftSource?: HistoryDraftSource;
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
