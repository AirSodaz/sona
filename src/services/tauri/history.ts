import type { HistoryItem } from '../../types/history';
import type { HistorySummaryPayload, TranscriptSegment } from '../../types/transcript';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface HistoryDraftHandle<TItem = Partial<HistoryItem>> {
  item: TItem;
  audioAbsolutePath: string;
}

export async function historyListItems(): Promise<Partial<HistoryItem>[]> {
  return invokeTauri<Partial<HistoryItem>[]>(TauriCommand.history.listItems);
}

export async function historyCreateLiveDraft<TItem extends HistoryItem>(
  item: TItem,
): Promise<HistoryDraftHandle<TItem>> {
  return invokeTauri<HistoryDraftHandle<TItem>>(TauriCommand.history.createLiveDraft, { item });
}

export async function historyCompleteLiveDraft(
  historyId: string,
  segments: TranscriptSegment[],
  previewText: string,
  searchContent: string,
  duration: number,
): Promise<Partial<HistoryItem>> {
  return invokeTauri<Partial<HistoryItem>>(TauriCommand.history.completeLiveDraft, {
    historyId,
    segments,
    previewText,
    searchContent,
    duration,
  });
}

export async function historySaveRecording(request: {
  item: Partial<HistoryItem>;
  segments: TranscriptSegment[];
  nativeAudioPath?: string;
  audioBytes?: number[];
}): Promise<Partial<HistoryItem>> {
  return invokeTauri<Partial<HistoryItem>>(TauriCommand.history.saveRecording, request);
}

export async function historySaveImportedFile(request: {
  item: Partial<HistoryItem>;
  segments: TranscriptSegment[];
  sourcePath: string;
}): Promise<Partial<HistoryItem>> {
  return invokeTauri<Partial<HistoryItem>>(TauriCommand.history.saveImportedFile, request);
}

export async function historyDeleteItems(ids: string[]): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.deleteItems, { ids });
}

export async function historyLoadTranscript(filename: string): Promise<unknown> {
  return invokeTauri<unknown>(TauriCommand.history.loadTranscript, { filename });
}

export async function historyUpdateTranscript(
  historyId: string,
  segments: TranscriptSegment[],
  previewText: string,
  searchContent: string,
): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.updateTranscript, {
    historyId,
    segments,
    previewText,
    searchContent,
  });
}

export async function historyUpdateItemMeta(
  historyId: string,
  updates: Partial<HistoryItem>,
): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.updateItemMeta, { historyId, updates });
}

export async function historyUpdateProjectAssignments(
  ids: string[],
  projectId: string | null,
): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.updateProjectAssignments, { ids, projectId });
}

export async function historyReassignProject(
  currentProjectId: string,
  nextProjectId: string | null,
): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.reassignProject, {
    currentProjectId,
    nextProjectId,
  });
}

export async function historyLoadSummary(
  historyId: string,
): Promise<HistorySummaryPayload | null> {
  return invokeTauri<HistorySummaryPayload | null>(TauriCommand.history.loadSummary, { historyId });
}

export async function historySaveSummary(
  historyId: string,
  summaryPayload: HistorySummaryPayload,
): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.saveSummary, { historyId, summaryPayload });
}

export async function historyDeleteSummary(historyId: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.deleteSummary, { historyId });
}

export async function historyResolveAudioPath(filename: string): Promise<string | null> {
  return invokeTauri<string | null>(TauriCommand.history.resolveAudioPath, { filename });
}

export async function historyOpenFolder(): Promise<void> {
  await invokeTauri<void>(TauriCommand.history.openFolder);
}
