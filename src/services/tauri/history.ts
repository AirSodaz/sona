import type { HistoryItem } from '../../types/history';
import type { HistorySummaryPayload, TranscriptSegment } from '../../types/transcript';
import type {
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from '../../types/transcriptSnapshot';
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

export async function historyListItems(): Promise<Partial<HistoryItem>[]> {
  return invokeTauri(TauriCommand.history.listItems);
}

export async function historyCreateLiveDraft<TItem extends HistoryItem>(
  item: TItem,
): Promise<HistoryDraftHandle<TItem>> {
  return invokeTauri(TauriCommand.history.createLiveDraft, { item }) as Promise<HistoryDraftHandle<TItem>>;
}

export async function historyCompleteLiveDraft(
  historyId: string,
  segments: TranscriptSegment[],
  previewText: string,
  searchContent: string,
  duration: number,
): Promise<Partial<HistoryItem>> {
  return invokeTauri(TauriCommand.history.completeLiveDraft, {
    historyId,
    segments,
    previewText,
    searchContent,
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

export async function historyLoadTranscript(filename: string): Promise<unknown> {
  return invokeTauri(TauriCommand.history.loadTranscript, { filename });
}

export async function historyUpdateTranscript(
  historyId: string,
  segments: TranscriptSegment[],
  previewText: string,
  searchContent: string,
): Promise<void> {
  await invokeTauri(TauriCommand.history.updateTranscript, {
    historyId,
    segments,
    previewText,
    searchContent,
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

export async function historyResolveAudioPath(filename: string): Promise<string | null> {
  return invokeTauri(TauriCommand.history.resolveAudioPath, { filename });
}

export async function historyOpenFolder(): Promise<void> {
  await invokeTauri(TauriCommand.history.openFolder);
}
