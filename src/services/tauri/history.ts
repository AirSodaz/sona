import type { HistoryItem } from '../../types/history';
import type { HistorySummaryPayload, TranscriptSegment } from '../../types/transcript';
import type {
  TranscriptDiffRow,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from '../../types/transcriptSnapshot';
import type { WorkspaceItemSearchMatch } from '../../utils/workspaceSearch';
import { TauriCommand } from './commands';
import type { TauriCommandArgs, TauriCommandResult } from './contracts';
import { invokeTauri } from './invoke';

type HistoryDraftTransportHandle = TauriCommandResult<typeof TauriCommand.history.createLiveDraft>;
type HistorySaveRecordingRequest = TauriCommandArgs<typeof TauriCommand.history.saveRecording>;
type HistorySaveImportedFileRequest = TauriCommandArgs<typeof TauriCommand.history.saveImportedFile>;

export interface HistoryDraftHandle<TItem = Partial<HistoryItem>>
  extends Omit<HistoryDraftTransportHandle, 'item'> {
  item: TItem;
}

export type HistoryWorkspaceQueryScope =
  | { kind: 'all' }
  | { kind: 'inbox' }
  | { kind: 'project'; projectId: string };

export interface HistoryWorkspaceQueryRequest {
  scope: HistoryWorkspaceQueryScope;
  query: string;
  filterType: 'all' | 'recording' | 'batch';
  dateFilter: 'all' | 'today' | 'week' | 'month';
  sortOrder: 'newest' | 'oldest' | 'duration_desc' | 'duration_asc' | 'title_asc';
}

export interface HistoryWorkspaceQueryResult {
  filteredItems: HistoryItem[];
  scopedItems: HistoryItem[];
  scopedItemIds: string[];
  searchMatchByItemId: Record<string, WorkspaceItemSearchMatch | null>;
  summary: {
    totalItems: number;
    totalDuration: number;
    latestTimestamp: number | null;
    recordingCount: number;
    batchCount: number;
  };
  itemCounts: {
    inbox: number;
    byProjectId: Record<string, number>;
  };
}

export async function historyListItems(): Promise<Partial<HistoryItem>[]> {
  return invokeTauri(TauriCommand.history.listItems);
}

export async function historyCreateLiveDraft(
  id: string | null,
  audioExtension: string,
  projectId: string | null,
  icon: string | null,
): Promise<HistoryDraftHandle> {
  return invokeTauri(TauriCommand.history.createLiveDraft, {
    id,
    audioExtension,
    projectId,
    icon,
  });
}

export async function historyCompleteLiveDraft(
  historyId: string,
  segments: TranscriptSegment[],
  duration: number,
): Promise<Partial<HistoryItem>> {
  return invokeTauri(TauriCommand.history.completeLiveDraft, {
    historyId,
    segments,
    duration,
  });
}

export async function historySaveRecording(
  request: HistorySaveRecordingRequest,
): Promise<Partial<HistoryItem>> {
  return invokeTauri(TauriCommand.history.saveRecording, request);
}

export async function historySaveImportedFile(
  request: HistorySaveImportedFileRequest,
): Promise<Partial<HistoryItem>> {
  return invokeTauri(TauriCommand.history.saveImportedFile, request);
}

export async function historyDeleteItems(ids: string[]): Promise<void> {
  await invokeTauri(TauriCommand.history.deleteItems, { ids });
}

export async function historyLoadTranscript(historyId: string): Promise<TranscriptSegment[] | null> {
  return invokeTauri(TauriCommand.history.loadTranscript, { historyId });
}

export async function historyUpdateTranscript(
  historyId: string,
  segments: TranscriptSegment[],
): Promise<Partial<HistoryItem>> {
  return invokeTauri(TauriCommand.history.updateTranscript, {
    historyId,
    segments,
  });
}

export async function historyCreateTranscriptSnapshot(
  historyId: string,
  reason: TranscriptSnapshotReason,
  segments: TranscriptSegment[],
): Promise<TranscriptSnapshotMetadata> {
  return invokeTauri(TauriCommand.history.createTranscriptSnapshot, {
    historyId,
    reason,
    segments,
  });
}

export async function historyListTranscriptSnapshots(
  historyId: string,
): Promise<TranscriptSnapshotMetadata[]> {
  return invokeTauri(TauriCommand.history.listTranscriptSnapshots, { historyId });
}

export async function historyLoadTranscriptSnapshot(
  historyId: string,
  snapshotId: string,
): Promise<TranscriptSnapshotRecord | null> {
  return invokeTauri(TauriCommand.history.loadTranscriptSnapshot, { historyId, snapshotId });
}

export async function historyBuildTranscriptDiff(
  snapshotSegments: TranscriptSegment[],
  currentSegments: TranscriptSegment[],
): Promise<{ rows: TranscriptDiffRow[]; changedCount: number }> {
  return invokeTauri(TauriCommand.history.buildTranscriptDiff, {
    snapshotSegments,
    currentSegments,
  });
}

export async function historyRestoreTranscriptDiffRows(
  rows: TranscriptDiffRow[],
  selectedRowIds: Iterable<string>,
): Promise<TranscriptSegment[]> {
  return invokeTauri(TauriCommand.history.restoreTranscriptDiffRows, {
    rows,
    selectedRowIds: Array.from(selectedRowIds),
  });
}

export async function historyUpdateItemMeta(
  historyId: string,
  updates: Partial<HistoryItem>,
): Promise<void> {
  await invokeTauri(TauriCommand.history.updateItemMeta, { historyId, updates });
}

export async function historyUpdateProjectAssignments(
  ids: string[],
  projectId: string | null,
): Promise<void> {
  await invokeTauri(TauriCommand.history.updateProjectAssignments, { ids, projectId });
}

export async function historyReassignProject(
  currentProjectId: string,
  nextProjectId: string | null,
): Promise<void> {
  await invokeTauri(TauriCommand.history.reassignProject, {
    currentProjectId,
    nextProjectId,
  });
}

export async function historyLoadSummary(
  historyId: string,
): Promise<HistorySummaryPayload | null> {
  return invokeTauri(TauriCommand.history.loadSummary, { historyId });
}

export async function historySaveSummary(
  historyId: string,
  summaryPayload: HistorySummaryPayload,
): Promise<void> {
  await invokeTauri(TauriCommand.history.saveSummary, { historyId, summaryPayload });
}

export async function historyDeleteSummary(historyId: string): Promise<void> {
  await invokeTauri(TauriCommand.history.deleteSummary, { historyId });
}

export async function historyResolveAudioPath(historyId: string): Promise<string | null> {
  return invokeTauri(TauriCommand.history.resolveAudioPath, { historyId });
}

export async function historyQueryWorkspace(
  request: HistoryWorkspaceQueryRequest,
): Promise<HistoryWorkspaceQueryResult> {
  return invokeTauri(TauriCommand.history.queryWorkspace, request);
}

export async function historyOpenFolder(): Promise<void> {
  await invokeTauri(TauriCommand.history.openFolder);
}
