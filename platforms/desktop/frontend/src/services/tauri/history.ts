import {
  normalizeHistoryItemRecord,
  type HistoryAudioCleanupReport,
  type HistoryItem,
} from "../../types/history";
import type {
  HistoryItemMetaPatch_Serialize,
  HistoryItemRecord,
  HistoryWorkspaceQueryRequest as CoreHistoryWorkspaceQueryRequest,
  HistoryWorkspaceQueryResult as CoreHistoryWorkspaceQueryResult,
  HistoryWorkspaceScope,
  TranscriptDiffResult_Serialize,
  TranscriptDiffRow_Serialize,
  TranscriptSegment_Serialize,
  TranscriptSnapshotRecord_Serialize,
} from "../../bindings";
import type {
  HistorySummaryPayload,
  TranscriptSegment,
} from "../../types/transcript";
import type {
  TranscriptDiffResult,
  TranscriptDiffRow,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from "../../types/transcriptSnapshot";
import {
  normalizeSpeakerAttribution,
  normalizeSpeakerTag,
} from "../../types/speaker";
import type { WorkspaceItemSearchMatch } from "../../utils/workspaceSearch";
import { TauriCommand } from "./commands";
import type { TauriCommandArgs, TauriCommandResult } from "./contracts";
import { invokeTauri } from "./invoke";

type HistoryDraftTransportHandle = TauriCommandResult<
  typeof TauriCommand.history.createLiveDraft
>;
type CoreHistorySaveRecordingRequest = TauriCommandArgs<
  typeof TauriCommand.history.saveRecording
>;
type CoreHistorySaveImportedFileRequest = TauriCommandArgs<
  typeof TauriCommand.history.saveImportedFile
>;
type HistorySaveRecordingRequest = CoreHistorySaveRecordingRequest & {
  /** @deprecated */
  projectId?: string | null;
};
type HistorySaveImportedFileRequest = CoreHistorySaveImportedFileRequest & {
  /** @deprecated */
  projectId?: string | null;
};
type HistoryAudioCleanupRequest = TauriCommandArgs<
  typeof TauriCommand.history.cleanupAudio
>;

function normalizeTranscriptSegment(segment: TranscriptSegment_Serialize): TranscriptSegment {
  return {
    id: segment.id,
    text: segment.text,
    start: segment.start,
    end: segment.end,
    isFinal: segment.isFinal,
    timing: segment.timing ?? undefined,
    tokens: segment.tokens ?? undefined,
    timestamps: segment.timestamps ?? undefined,
    durations: segment.durations ?? undefined,
    translation: segment.translation ?? undefined,
    speaker: normalizeSpeakerTag(segment.speaker) ?? undefined,
    speakerAttribution:
      normalizeSpeakerAttribution(segment.speakerAttribution) ?? undefined,
  };
}

function normalizeTranscriptDiffRow(row: TranscriptDiffRow_Serialize): TranscriptDiffRow {
  return {
    ...row,
    snapshotSegment: row.snapshotSegment
      ? normalizeTranscriptSegment(row.snapshotSegment)
      : undefined,
    currentSegment: row.currentSegment
      ? normalizeTranscriptSegment(row.currentSegment)
      : undefined,
  };
}

export interface HistoryDraftHandle<TItem = HistoryItemRecord> extends Omit<
  HistoryDraftTransportHandle,
  "item"
> {
  item: TItem;
}

export type HistoryWorkspaceQueryScope = HistoryWorkspaceScope
  | { kind: 'inbox' }
  | { kind: 'project'; projectId: string };
export type HistoryWorkspaceQueryRequest = Omit<CoreHistoryWorkspaceQueryRequest, 'scope'> & {
  scope: HistoryWorkspaceQueryScope;
};
export type HistoryWorkspaceQueryResult = Omit<
  CoreHistoryWorkspaceQueryResult,
  "filteredItems" | "searchMatchByItemId" | "itemCounts"
> & {
  filteredItems: HistoryItem[];
  searchMatchByItemId: Record<string, WorkspaceItemSearchMatch | null>;
  itemCounts: {
    untagged?: number;
    trash?: number;
    byTagId?: Record<string, number>;
    /** @deprecated Compatibility fields for old tests and cached payloads. */
    inbox?: number;
    byProjectId?: Record<string, number>;
  };
};

export async function historyListItems(opts?: {
  limit?: number;
  offset?: number;
}): Promise<HistoryItemRecord[]> {
  return invokeTauri(TauriCommand.history.listItems, opts ?? {});
}

export async function historyCreateLiveDraft(
  id: string | null,
  audioExtension: string,
  tagIds: string[] | string | null,
  icon: string | null,
): Promise<HistoryDraftHandle> {
  return invokeTauri(TauriCommand.history.createLiveDraft, {
    id,
    audioExtension,
    tagIds: Array.isArray(tagIds) ? tagIds : tagIds ? [tagIds] : [],
    icon,
  });
}

export async function historyCompleteLiveDraft(
  historyId: string,
  segments: TranscriptSegment[],
  duration: number,
): Promise<HistoryItemRecord> {
  return invokeTauri(TauriCommand.history.completeLiveDraft, {
    historyId,
    segments,
    duration,
  });
}

export async function historySaveRecording(
  request: HistorySaveRecordingRequest,
): Promise<HistoryItemRecord> {
  const { projectId, ...rest } = request;
  return invokeTauri(TauriCommand.history.saveRecording, {
    ...rest,
    tagIds: rest.tagIds ?? (projectId ? [projectId] : []),
  });
}

export async function historySaveImportedFile(
  request: HistorySaveImportedFileRequest,
): Promise<HistoryItemRecord> {
  const { projectId, ...rest } = request;
  return invokeTauri(TauriCommand.history.saveImportedFile, {
    ...rest,
    tagIds: rest.tagIds ?? (projectId ? [projectId] : []),
  });
}

export async function historyDeleteItems(ids: string[]): Promise<void> {
  await invokeTauri(TauriCommand.history.deleteItems, { ids });
}

export async function historyTrashItems(ids: string[], deletedAt = Date.now()): Promise<void> {
  await invokeTauri(TauriCommand.history.trashItems, { ids, deletedAt });
}

export async function historyRestoreItems(ids: string[]): Promise<void> {
  await invokeTauri(TauriCommand.history.restoreItems, { ids });
}

export async function historyPurgeItems(ids: string[]): Promise<void> {
  await invokeTauri(TauriCommand.history.purgeItems, { ids });
}

export async function historyLoadTranscript(
  historyId: string,
): Promise<TranscriptSegment[] | null> {
  const segments = await invokeTauri(TauriCommand.history.loadTranscript, { historyId });
  return segments?.map(normalizeTranscriptSegment) ?? null;
}

export async function historyUpdateTranscript(
  historyId: string,
  segments: TranscriptSegment[],
): Promise<HistoryItemRecord> {
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
  return invokeTauri(TauriCommand.history.listTranscriptSnapshots, {
    historyId,
  });
}

export async function historyLoadTranscriptSnapshot(
  historyId: string,
  snapshotId: string,
): Promise<TranscriptSnapshotRecord | null> {
  const record: TranscriptSnapshotRecord_Serialize | null = await invokeTauri(
    TauriCommand.history.loadTranscriptSnapshot,
    { historyId, snapshotId },
  );
  return record
    ? { ...record, segments: record.segments.map(normalizeTranscriptSegment) }
    : null;
}

export async function historyBuildTranscriptDiff(
  snapshotSegments: TranscriptSegment[],
  currentSegments: TranscriptSegment[],
): Promise<{ rows: TranscriptDiffRow[]; changedCount: number }> {
  const result: TranscriptDiffResult_Serialize = await invokeTauri(
    TauriCommand.history.buildTranscriptDiff,
    { snapshotSegments, currentSegments },
  );
  return {
    ...result,
    rows: result.rows.map(normalizeTranscriptDiffRow),
  } satisfies TranscriptDiffResult;
}

export async function historyRestoreTranscriptDiffRows(
  rows: TranscriptDiffRow[],
  selectedRowIds: Iterable<string>,
): Promise<TranscriptSegment[]> {
  const segments = await invokeTauri(TauriCommand.history.restoreTranscriptDiffRows, {
    rows,
    selectedRowIds: Array.from(selectedRowIds),
  });
  return segments.map(normalizeTranscriptSegment);
}

export async function historyUpdateItemMeta(
  historyId: string,
  updates: HistoryItemMetaPatch_Serialize,
): Promise<void> {
  await invokeTauri(TauriCommand.history.updateItemMeta, {
    historyId,
    updates,
  });
}

export async function historyUpdateProjectAssignments(
  ids: string[],
  projectId: string | null,
): Promise<void> {
  await invokeTauri(TauriCommand.history.updateProjectAssignments, {
    ids,
    projectId,
  });
}

export async function historyUpdateTagAssignments(
  ids: string[],
  addTagIds: string[],
  removeTagIds: string[],
): Promise<void> {
  await invokeTauri(TauriCommand.history.updateTagAssignments, {
    ids,
    addTagIds,
    removeTagIds,
  });
}

export async function historyReplaceTagAssignments(
  ids: string[],
  tagIds: string[],
): Promise<void> {
  await invokeTauri(TauriCommand.history.replaceTagAssignments, { ids, tagIds });
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
  await invokeTauri(TauriCommand.history.saveSummary, {
    historyId,
    summaryPayload,
  });
}

export async function historyDeleteSummary(historyId: string): Promise<void> {
  await invokeTauri(TauriCommand.history.deleteSummary, { historyId });
}

export async function historyResolveAudioPath(
  historyId: string,
): Promise<string | null> {
  return invokeTauri(TauriCommand.history.resolveAudioPath, { historyId });
}

export async function historyPreviewAudioCleanup(
  request: HistoryAudioCleanupRequest,
): Promise<HistoryAudioCleanupReport> {
  return invokeTauri(TauriCommand.history.previewAudioCleanup, request);
}

export async function historyCleanupAudio(
  request: HistoryAudioCleanupRequest,
): Promise<HistoryAudioCleanupReport> {
  return invokeTauri(TauriCommand.history.cleanupAudio, request);
}

export async function historyQueryWorkspace(
  request: HistoryWorkspaceQueryRequest,
): Promise<HistoryWorkspaceQueryResult> {
  const result = await invokeTauri(
    TauriCommand.history.queryWorkspace,
    request as CoreHistoryWorkspaceQueryRequest,
  );
  const searchMatchByItemId = Object.fromEntries(
    Object.entries(result.searchMatchByItemId).map(([itemId, match]) => {
      if (!match) return [itemId, null];
      const matchedField =
        match.matchedField === "title" || match.matchedField === "previewText"
          ? match.matchedField
          : "searchContent";
      return [itemId, { ...match, matchedField } satisfies WorkspaceItemSearchMatch];
    }),
  );
  return {
    ...result,
    filteredItems: result.filteredItems.map(normalizeHistoryItemRecord),
    searchMatchByItemId,
    itemCounts: {
      ...result.itemCounts,
      untagged: result.itemCounts.untagged,
      trash: result.itemCounts.trash,
      byTagId: result.itemCounts.byTagId,
    },
  };
}

export async function historyOpenFolder(): Promise<void> {
  await invokeTauri(TauriCommand.history.openFolder);
}
