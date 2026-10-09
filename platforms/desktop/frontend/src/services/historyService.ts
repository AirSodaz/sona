import type {
  HistoryItemRecord,
  TranscriptDiffResult_Serialize,
  TranscriptSegment_Serialize,
  TranscriptSnapshotRecord_Serialize,
} from '../bindings';
import {
  getPlatform,
  type IAssetPort,
  type ITransport,
  type PlatformContext,
  TauriCommand,
} from '../platform';
import {
  type HistoryAudioCleanupReport,
  type HistoryItem,
  normalizeHistoryItemRecord as normalizeHistoryItem,
} from '../types/history';
import type { HistorySummaryPayload, TranscriptSegment } from '../types/transcript';
import type {
  TranscriptDiffResult,
  TranscriptDiffRow,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from '../types/transcriptSnapshot';
import { logger } from '../utils/logger';
import {
  normalizeTranscriptDiffRow,
  normalizeTranscriptSegment,
  toHistorySummaryPayloadTransport,
  toTranscriptDiffRowTransport,
  toTranscriptSegmentTransport,
} from './historyTransportUtils';

export interface LiveRecordingDraftHandle {
  item: HistoryItem;
  audioAbsolutePath: string;
}

export type TranscriptEditCommitResult =
  | { status: 'unchanged' }
  | { status: 'committed'; item: HistoryItem; snapshot: TranscriptSnapshotMetadata }
  | { status: 'conflict'; currentSegments: TranscriptSegment[] };

function inferAudioExtensionFromPath(filePath: string, fallback: string): string {
  const fileName = filePath.split(/[/\\]/).pop() || '';
  const extensionIndex = fileName.lastIndexOf('.');
  const extension = extensionIndex >= 0 ? fileName.slice(extensionIndex + 1).trim() : '';
  return extension || fallback;
}

function inferAudioExtensionFromBlob(blob: Blob): string {
  const mimeType = blob.type.toLowerCase();
  if (mimeType.includes('mp4')) return 'm4a';
  if (mimeType.includes('aac')) return 'aac';
  if (mimeType.includes('ogg')) return 'ogg';
  if (mimeType.includes('wav')) return 'wav';
  return 'webm';
}

interface SaveRecordingInternalRequest {
  segments: TranscriptSegment[];
  duration: number;
  projectId?: string | null;
  audioBytes?: number[] | null;
  nativeAudioPath?: string | null;
  audioExtension?: string | null;
}

interface SaveImportedFileInternalRequest {
  sourcePath: string;
  segments: TranscriptSegment[];
  duration: number;
  projectId?: string | null;
  convertedSourcePath?: string | null;
  id?: string | null;
}

export interface IHistoryOperations {
  listItems: (opts?: { limit?: number; offset?: number }) => Promise<unknown[]>;
  createLiveDraft: (
    id: string | null,
    audioExtension: string,
    projectId: string | null,
    icon: string | null
  ) => Promise<{ item: unknown; audioAbsolutePath?: string }>;
  completeLiveDraft: (
    historyId: string,
    segments: TranscriptSegment[],
    duration: number
  ) => Promise<unknown>;
  cleanupAudio: (request: {
    retentionDays: number | null;
    excludeHistoryId: string | null;
  }) => Promise<HistoryAudioCleanupReport>;
  deleteItems: (ids: string[]) => Promise<void>;
  purgeItems: (ids: string[]) => Promise<void>;
  restoreItems: (ids: string[]) => Promise<void>;
  trashItems: (ids: string[], deletedAt?: number) => Promise<void>;
  deleteSummary: (historyId: string) => Promise<void>;
  createTranscriptSnapshot: (
    historyId: string,
    reason: TranscriptSnapshotReason,
    segments: TranscriptSegment[]
  ) => Promise<TranscriptSnapshotMetadata>;
  commitTranscriptEdit: (
    historyId: string,
    editSessionId: string,
    baseSegments: TranscriptSegment[],
    editedSegments: TranscriptSegment[]
  ) => Promise<TranscriptEditCommitResult>;
  buildTranscriptDiff: (
    snapshotSegments: TranscriptSegment[],
    currentSegments: TranscriptSegment[]
  ) => Promise<TranscriptDiffResult>;
  loadSummary: (historyId: string) => Promise<HistorySummaryPayload | null>;
  loadTranscript: (historyId: string) => Promise<TranscriptSegment[] | null>;
  listTranscriptSnapshots: (historyId: string) => Promise<TranscriptSnapshotMetadata[]>;
  loadTranscriptSnapshot: (
    historyId: string,
    snapshotId: string
  ) => Promise<TranscriptSnapshotRecord | null>;
  openFolder: () => Promise<void>;
  previewAudioCleanup: (request: {
    retentionDays: number | null;
    excludeHistoryId: string | null;
  }) => Promise<HistoryAudioCleanupReport>;
  reassignProject: (currentProjectId: string, nextProjectId: string | null) => Promise<void>;
  restoreTranscriptDiffRows: (
    rows: TranscriptDiffRow[],
    selectedRowIds: Iterable<string>
  ) => Promise<TranscriptSegment[]>;
  resolveAudioPath: (historyId: string) => Promise<string | null>;
  saveImportedFile: (request: SaveImportedFileInternalRequest) => Promise<unknown>;
  saveImportedFileToProject?: (request: SaveImportedFileInternalRequest) => Promise<unknown>;
  saveRecording: (request: SaveRecordingInternalRequest) => Promise<unknown>;
  saveRecordingToProject?: (request: SaveRecordingInternalRequest) => Promise<unknown>;
  saveSummary: (historyId: string, summaryPayload: HistorySummaryPayload) => Promise<void>;
  updateItemMeta: (id: string, updates: Partial<HistoryItem>) => Promise<void>;
  updateProjectAssignments: (ids: string[], projectId: string | null) => Promise<void>;
  updateTranscript: (historyId: string, segments: TranscriptSegment[]) => Promise<unknown>;
  convertAudioSrc: (filePath: string) => string;
}

export interface HistoryServiceLegacyPorts {
  historyListItems: (opts?: { limit?: number; offset?: number }) => Promise<unknown[]>;
  historyCreateLiveDraft: (
    id: string | null,
    audioExtension: string,
    projectId: string | null,
    icon: string | null
  ) => Promise<{ item: unknown; audioAbsolutePath?: string }>;
  historyCompleteLiveDraft: (
    historyId: string,
    segments: TranscriptSegment[],
    duration: number
  ) => Promise<unknown>;
  historyCleanupAudio: (request: {
    retentionDays: number | null;
    excludeHistoryId: string | null;
  }) => Promise<HistoryAudioCleanupReport>;
  historyDeleteItems: (ids: string[]) => Promise<void>;
  historyPurgeItems: (ids: string[]) => Promise<void>;
  historyRestoreItems: (ids: string[]) => Promise<void>;
  historyTrashItems: (ids: string[], deletedAt?: number) => Promise<void>;
  historyDeleteSummary: (historyId: string) => Promise<void>;
  historyCreateTranscriptSnapshot: (
    historyId: string,
    reason: TranscriptSnapshotReason,
    segments: TranscriptSegment[]
  ) => Promise<TranscriptSnapshotMetadata>;
  historyCommitTranscriptEdit: (
    historyId: string,
    editSessionId: string,
    baseSegments: TranscriptSegment[],
    editedSegments: TranscriptSegment[]
  ) => Promise<
    | { status: 'unchanged' }
    | {
        status: 'committed';
        item: HistoryItemRecord;
        snapshot: TranscriptSnapshotMetadata;
      }
    | { status: 'conflict'; currentSegments: TranscriptSegment[] }
  >;
  historyBuildTranscriptDiff: (
    snapshotSegments: TranscriptSegment[],
    currentSegments: TranscriptSegment[]
  ) => Promise<TranscriptDiffResult>;
  historyLoadSummary: (historyId: string) => Promise<HistorySummaryPayload | null>;
  historyLoadTranscript: (historyId: string) => Promise<TranscriptSegment[] | null>;
  historyListTranscriptSnapshots: (historyId: string) => Promise<TranscriptSnapshotMetadata[]>;
  historyLoadTranscriptSnapshot: (
    historyId: string,
    snapshotId: string
  ) => Promise<TranscriptSnapshotRecord | null>;
  historyOpenFolder: () => Promise<void>;
  historyPreviewAudioCleanup: (request: {
    retentionDays: number | null;
    excludeHistoryId: string | null;
  }) => Promise<HistoryAudioCleanupReport>;
  historyReassignProject: (currentProjectId: string, nextProjectId: string | null) => Promise<void>;
  historyRestoreTranscriptDiffRows: (
    rows: TranscriptDiffRow[],
    selectedRowIds: Iterable<string>
  ) => Promise<TranscriptSegment[]>;
  historyResolveAudioPath: (historyId: string) => Promise<string | null>;
  historySaveImportedFile: (request: SaveImportedFileInternalRequest) => Promise<unknown>;
  historySaveImportedFileToProject?: (request: SaveImportedFileInternalRequest) => Promise<unknown>;
  historySaveRecording: (request: SaveRecordingInternalRequest) => Promise<unknown>;
  historySaveRecordingToProject?: (request: SaveRecordingInternalRequest) => Promise<unknown>;
  historySaveSummary: (historyId: string, summaryPayload: HistorySummaryPayload) => Promise<void>;
  historyUpdateItemMeta: (id: string, updates: Partial<HistoryItem>) => Promise<void>;
  historyUpdateProjectAssignments: (ids: string[], projectId: string | null) => Promise<void>;
  historyUpdateTranscript: (historyId: string, segments: TranscriptSegment[]) => Promise<unknown>;
  convertManagedAudioFileSrc: (filePath: string) => string;
}

export interface HistoryServiceTransportDependencies {
  transport: ITransport;
  assets?: IAssetPort;
}

export type HistoryServiceInput =
  | HistoryServiceTransportDependencies
  | PlatformContext
  | HistoryServiceLegacyPorts;

export type HistoryServicePorts = HistoryServiceInput;

function createOperationsFromLegacyPorts(ports: HistoryServiceLegacyPorts): IHistoryOperations {
  const saveImportedFileToProject = ports.historySaveImportedFileToProject;
  const saveRecordingToProject = ports.historySaveRecordingToProject;
  return {
    listItems: (opts) => ports.historyListItems(opts),
    createLiveDraft: (id, audioExtension, projectId, icon) =>
      ports.historyCreateLiveDraft(id, audioExtension, projectId, icon),
    completeLiveDraft: (historyId, segments, duration) =>
      ports.historyCompleteLiveDraft(historyId, segments, duration),
    cleanupAudio: (request) => ports.historyCleanupAudio(request),
    deleteItems: (ids) => ports.historyDeleteItems(ids),
    purgeItems: (ids) => ports.historyPurgeItems(ids),
    restoreItems: (ids) => ports.historyRestoreItems(ids),
    trashItems: (ids, deletedAt) => ports.historyTrashItems(ids, deletedAt),
    deleteSummary: (historyId) => ports.historyDeleteSummary(historyId),
    createTranscriptSnapshot: (historyId, reason, segments) =>
      ports.historyCreateTranscriptSnapshot(historyId, reason, segments),
    commitTranscriptEdit: async (historyId, editSessionId, baseSegments, editedSegments) => {
      const result = await ports.historyCommitTranscriptEdit(
        historyId,
        editSessionId,
        baseSegments,
        editedSegments
      );
      if (result.status === 'committed') {
        return {
          status: 'committed',
          item: normalizeHistoryItem(result.item),
          snapshot: result.snapshot,
        };
      }
      return result;
    },
    buildTranscriptDiff: (snapshotSegments, currentSegments) =>
      ports.historyBuildTranscriptDiff(snapshotSegments, currentSegments),
    loadSummary: (historyId) => ports.historyLoadSummary(historyId),
    loadTranscript: (historyId) => ports.historyLoadTranscript(historyId),
    listTranscriptSnapshots: (historyId) => ports.historyListTranscriptSnapshots(historyId),
    loadTranscriptSnapshot: (historyId, snapshotId) =>
      ports.historyLoadTranscriptSnapshot(historyId, snapshotId),
    openFolder: () => ports.historyOpenFolder(),
    previewAudioCleanup: (request) => ports.historyPreviewAudioCleanup(request),
    reassignProject: (currentProjectId, nextProjectId) =>
      ports.historyReassignProject(currentProjectId, nextProjectId),
    restoreTranscriptDiffRows: (rows, selectedRowIds) =>
      ports.historyRestoreTranscriptDiffRows(rows, selectedRowIds),
    resolveAudioPath: (historyId) => ports.historyResolveAudioPath(historyId),
    saveImportedFile: (request) => ports.historySaveImportedFile(request),
    saveImportedFileToProject: saveImportedFileToProject
      ? (request) => saveImportedFileToProject(request)
      : undefined,
    saveRecording: (request) => ports.historySaveRecording(request),
    saveRecordingToProject: saveRecordingToProject
      ? (request) => saveRecordingToProject(request)
      : undefined,
    saveSummary: (historyId, summaryPayload) => ports.historySaveSummary(historyId, summaryPayload),
    updateItemMeta: (id, updates) => ports.historyUpdateItemMeta(id, updates),
    updateProjectAssignments: (ids, projectId) =>
      ports.historyUpdateProjectAssignments(ids, projectId),
    updateTranscript: (historyId, segments) => ports.historyUpdateTranscript(historyId, segments),
    convertAudioSrc: (filePath) => ports.convertManagedAudioFileSrc(filePath),
  };
}

function createOperationsFromTransport(
  transport: ITransport,
  assetPort?: IAssetPort
): IHistoryOperations {
  return {
    listItems: (opts) => transport.invoke(TauriCommand.history.listItems, opts ?? {}),
    createLiveDraft: (id, audioExtension, projectId, icon) =>
      transport.invoke(TauriCommand.history.createLiveDraft, {
        id,
        audioExtension,
        projectId,
        icon,
      }),
    completeLiveDraft: (historyId, segments, duration) =>
      transport.invoke(TauriCommand.history.completeLiveDraft, {
        historyId,
        segments: segments.map(toTranscriptSegmentTransport),
        duration,
      }),
    cleanupAudio: (request) => transport.invoke(TauriCommand.history.cleanupAudio, request),
    deleteItems: (ids) => transport.invoke(TauriCommand.history.deleteItems, { ids }),
    purgeItems: (ids) => transport.invoke(TauriCommand.history.purgeItems, { ids }),
    restoreItems: (ids) => transport.invoke(TauriCommand.history.restoreItems, { ids }),
    trashItems: (ids, deletedAt) =>
      transport.invoke(TauriCommand.history.trashItems, { ids, deletedAt }),
    deleteSummary: (historyId) =>
      transport.invoke(TauriCommand.history.deleteSummary, { historyId }),
    createTranscriptSnapshot: (historyId, reason, segments) =>
      transport.invoke(TauriCommand.history.createTranscriptSnapshot, {
        historyId,
        reason,
        segments: segments.map(toTranscriptSegmentTransport),
      }),
    commitTranscriptEdit: async (historyId, editSessionId, baseSegments, editedSegments) => {
      const result = await transport.invoke<{
        status: 'unchanged' | 'committed' | 'conflict';
        item?: unknown;
        snapshot?: TranscriptSnapshotMetadata;
        current_segments?: TranscriptSegment_Serialize[];
      }>(TauriCommand.history.commitTranscriptEdit, {
        historyId,
        editSessionId,
        baseSegments: baseSegments.map(toTranscriptSegmentTransport),
        editedSegments: editedSegments.map(toTranscriptSegmentTransport),
      });
      if (result.status === 'conflict') {
        return {
          status: 'conflict',
          currentSegments: (result.current_segments ?? []).map(normalizeTranscriptSegment),
        };
      }
      if (result.status === 'committed') {
        if (!result.item || !result.snapshot) {
          throw new Error('Committed edit result missing item or snapshot');
        }
        return {
          status: 'committed',
          item: normalizeHistoryItem(result.item as Partial<HistoryItemRecord>),
          snapshot: result.snapshot,
        };
      }
      return { status: 'unchanged' };
    },
    buildTranscriptDiff: async (snapshotSegments, currentSegments) => {
      const result = await transport.invoke<TranscriptDiffResult_Serialize>(
        TauriCommand.history.buildTranscriptDiff,
        {
          snapshotSegments: snapshotSegments.map(toTranscriptSegmentTransport),
          currentSegments: currentSegments.map(toTranscriptSegmentTransport),
        }
      );
      return {
        rows: result.rows.map(normalizeTranscriptDiffRow),
        changedCount: result.changedCount,
      };
    },
    loadSummary: (historyId) => transport.invoke(TauriCommand.history.loadSummary, { historyId }),
    loadTranscript: async (historyId) => {
      const segments = await transport.invoke<TranscriptSegment_Serialize[] | null>(
        TauriCommand.history.loadTranscript,
        { historyId }
      );
      return segments ? segments.map(normalizeTranscriptSegment) : null;
    },
    listTranscriptSnapshots: (historyId) =>
      transport.invoke(TauriCommand.history.listTranscriptSnapshots, { historyId }),
    loadTranscriptSnapshot: async (historyId, snapshotId) => {
      const record = await transport.invoke<TranscriptSnapshotRecord_Serialize | null>(
        TauriCommand.history.loadTranscriptSnapshot,
        { historyId, snapshotId }
      );
      return record
        ? { ...record, segments: record.segments.map(normalizeTranscriptSegment) }
        : null;
    },
    openFolder: () => transport.invoke(TauriCommand.history.openFolder),
    previewAudioCleanup: (request) =>
      transport.invoke(TauriCommand.history.previewAudioCleanup, request),
    reassignProject: (currentProjectId, nextProjectId) =>
      transport.invoke(TauriCommand.history.reassignProject, { currentProjectId, nextProjectId }),
    restoreTranscriptDiffRows: async (rows, selectedRowIds) => {
      const result = await transport.invoke<TranscriptSegment_Serialize[]>(
        TauriCommand.history.restoreTranscriptDiffRows,
        {
          rows: rows.map(toTranscriptDiffRowTransport),
          selectedRowIds: Array.from(selectedRowIds),
        }
      );
      return result.map(normalizeTranscriptSegment);
    },
    resolveAudioPath: (historyId) =>
      transport.invoke(TauriCommand.history.resolveAudioPath, { historyId }),
    saveImportedFile: (request) => {
      const { projectId, ...rest } = request;
      if (projectId !== undefined) {
        return transport.invoke(TauriCommand.history.saveImportedFileToProject, {
          ...rest,
          projectId: projectId ?? null,
          segments: rest.segments.map(toTranscriptSegmentTransport),
        });
      }
      return transport.invoke(TauriCommand.history.saveImportedFile, {
        ...rest,
        projectId: null,
        segments: rest.segments.map(toTranscriptSegmentTransport),
      });
    },
    saveImportedFileToProject: (request) =>
      transport.invoke(TauriCommand.history.saveImportedFileToProject, {
        ...request,
        segments: request.segments.map(toTranscriptSegmentTransport),
      }),
    saveRecording: (request) => {
      const { projectId, ...rest } = request;
      if (projectId !== undefined) {
        return transport.invoke(TauriCommand.history.saveRecordingToProject, {
          ...rest,
          projectId: projectId ?? null,
          segments: rest.segments.map(toTranscriptSegmentTransport),
        });
      }
      return transport.invoke(TauriCommand.history.saveRecording, {
        ...rest,
        projectId: null,
        segments: rest.segments.map(toTranscriptSegmentTransport),
      });
    },
    saveRecordingToProject: (request) =>
      transport.invoke(TauriCommand.history.saveRecordingToProject, {
        ...request,
        segments: request.segments.map(toTranscriptSegmentTransport),
      }),
    saveSummary: (historyId, summaryPayload) =>
      transport.invoke(TauriCommand.history.saveSummary, {
        historyId,
        summaryPayload: toHistorySummaryPayloadTransport(summaryPayload),
      }),
    updateItemMeta: (id, updates) =>
      transport.invoke(TauriCommand.history.updateItemMeta, { id, updates }),
    updateProjectAssignments: (ids, projectId) =>
      transport.invoke(TauriCommand.history.updateProjectAssignments, { ids, projectId }),
    updateTranscript: (historyId, segments) =>
      transport.invoke(TauriCommand.history.updateTranscript, {
        historyId,
        segments: segments.map(toTranscriptSegmentTransport),
      }),
    convertAudioSrc: (filePath) => (assetPort ? assetPort.convertAudioSrc(filePath) : filePath),
  };
}

export class HistoryService {
  private readonly ops: IHistoryOperations;

  constructor(input?: HistoryServiceInput) {
    if (!input) {
      const platform = getPlatform();
      this.ops = createOperationsFromTransport(platform.transport, platform.ports.assets);
    } else if ('transport' in input) {
      const assets =
        'assets' in input && input.assets
          ? input.assets
          : 'ports' in input && input.ports && 'assets' in input.ports
            ? input.ports.assets
            : undefined;
      this.ops = createOperationsFromTransport(input.transport, assets);
    } else if ('historyListItems' in input) {
      this.ops = createOperationsFromLegacyPorts(input as HistoryServiceLegacyPorts);
    } else {
      const platform = getPlatform();
      this.ops = createOperationsFromTransport(platform.transport, platform.ports.assets);
    }
  }

  async getAll(): Promise<HistoryItem[]> {
    const items = await this.ops.listItems();
    return Array.isArray(items)
      ? items.map((item) => normalizeHistoryItem(item as Partial<HistoryItemRecord>))
      : [];
  }

  async createLiveRecordingDraft(
    audioExtension: string,
    projectId: string | null = null,
    icon: string | null = 'system:mic',
    id?: string
  ): Promise<LiveRecordingDraftHandle> {
    const result = await this.ops.createLiveDraft(id ?? null, audioExtension, projectId, icon);

    return {
      item: normalizeHistoryItem(result?.item as Partial<HistoryItemRecord>),
      audioAbsolutePath: result?.audioAbsolutePath || '',
    };
  }

  async completeLiveRecordingDraft(
    historyId: string,
    segments: TranscriptSegment[],
    duration: number
  ): Promise<HistoryItem> {
    const item = await this.ops.completeLiveDraft(historyId, segments, duration);
    return normalizeHistoryItem(item as Partial<HistoryItemRecord>);
  }

  async discardLiveRecordingDraft(historyId: string): Promise<void> {
    await this.ops.purgeItems([historyId]);
  }

  async saveNativeRecording(
    absoluteWavPath: string,
    segments: TranscriptSegment[],
    duration: number,
    projectId: string | null = null
  ): Promise<HistoryItem | null> {
    logger.info('[History] Saving native recording...', {
      absoluteWavPath,
      segments: segments.length,
      duration,
    });
    if (!segments || segments.length === 0) {
      logger.info('[History] Empty transcript, skipping save.');
      return null;
    }
    const item = await this.ops.saveRecording({
      segments,
      duration,
      projectId,
      nativeAudioPath: absoluteWavPath,
      audioExtension: inferAudioExtensionFromPath(absoluteWavPath, 'wav'),
    });
    return normalizeHistoryItem(item as Partial<HistoryItemRecord>);
  }

  async saveNativeRecordingToProject(
    absoluteWavPath: string,
    segments: TranscriptSegment[],
    duration: number,
    projectId: string | null
  ): Promise<HistoryItem | null> {
    if (this.ops.saveRecordingToProject) {
      const res = await this.ops.saveRecordingToProject({
        segments,
        duration,
        projectId,
        nativeAudioPath: absoluteWavPath,
        audioExtension: inferAudioExtensionFromPath(absoluteWavPath, 'wav'),
      });
      return normalizeHistoryItem(res as Partial<HistoryItemRecord>);
    }
    return this.saveNativeRecording(absoluteWavPath, segments, duration, projectId);
  }

  async saveRecording(
    audioBlob: Blob,
    segments: TranscriptSegment[],
    duration: number,
    projectId: string | null = null
  ): Promise<HistoryItem | null> {
    logger.info('[History] Saving recording...', {
      blobSize: audioBlob.size,
      segments: segments.length,
      duration,
    });
    if (!segments || segments.length === 0) {
      logger.info('[History] Empty transcript, skipping save.');
      return null;
    }
    const audioBytes = Array.from(new Uint8Array(await audioBlob.arrayBuffer()));
    const item = await this.ops.saveRecording({
      segments,
      duration,
      projectId,
      audioBytes,
      audioExtension: inferAudioExtensionFromBlob(audioBlob),
    });
    return normalizeHistoryItem(item as Partial<HistoryItemRecord>);
  }

  async saveRecordingToProject(
    audioBlob: Blob,
    segments: TranscriptSegment[],
    duration: number,
    projectId: string | null
  ): Promise<HistoryItem | null> {
    return this.saveRecording(audioBlob, segments, duration, projectId);
  }

  async saveImportedFile(
    filePath: string,
    segments: TranscriptSegment[],
    duration: number = 0,
    convertedFilePath?: string,
    projectId: string | null = null,
    id?: string
  ): Promise<HistoryItem | null> {
    logger.info('[History] Saving imported file...', { filePath, segments: segments.length });
    if (!segments || segments.length === 0) {
      logger.info('[History] Empty transcript, skipping save.');
      return null;
    }
    const item = await this.ops.saveImportedFile({
      sourcePath: filePath,
      segments,
      duration,
      projectId,
      convertedSourcePath: convertedFilePath,
      id: id ?? null,
    });
    return normalizeHistoryItem(item as Partial<HistoryItemRecord>);
  }

  async saveImportedFileToProject(
    filePath: string,
    segments: TranscriptSegment[],
    duration: number,
    convertedFilePath: string | undefined,
    projectId: string | null,
    id?: string
  ): Promise<HistoryItem | null> {
    if (this.ops.saveImportedFileToProject) {
      const res = await this.ops.saveImportedFileToProject({
        id,
        sourcePath: filePath,
        segments,
        duration,
        projectId,
        convertedSourcePath: convertedFilePath,
      });
      return normalizeHistoryItem(res as Partial<HistoryItemRecord>);
    }
    return this.saveImportedFile(filePath, segments, duration, convertedFilePath, projectId, id);
  }

  async deleteRecording(id: string): Promise<void> {
    await this.ops.deleteItems([id]);
  }

  async deleteRecordings(ids: string[]): Promise<void> {
    try {
      await this.ops.deleteItems(ids);
    } catch (error) {
      logger.error('Failed to delete recordings:', error);
      throw error;
    }
  }

  async trashRecordings(ids: string[]): Promise<void> {
    if (ids.length > 0) {
      await this.ops.trashItems(ids);
    }
  }

  async restoreRecordings(ids: string[]): Promise<void> {
    if (ids.length > 0) {
      await this.ops.restoreItems(ids);
    }
  }

  async purgeRecordings(ids: string[]): Promise<void> {
    if (ids.length > 0) {
      await this.ops.purgeItems(ids);
    }
  }

  async loadTranscript(historyId: string): Promise<TranscriptSegment[] | null> {
    return await this.ops.loadTranscript(historyId);
  }

  async updateTranscript(historyId: string, segments: TranscriptSegment[]): Promise<HistoryItem> {
    try {
      const item = await this.ops.updateTranscript(historyId, segments);
      return normalizeHistoryItem(item as Partial<HistoryItemRecord>);
    } catch (error) {
      logger.error('[History] Failed to update transcript:', error);
      throw error;
    }
  }

  async commitTranscriptEdit(
    historyId: string,
    editSessionId: string,
    baseSegments: TranscriptSegment[],
    editedSegments: TranscriptSegment[]
  ): Promise<TranscriptEditCommitResult> {
    return await this.ops.commitTranscriptEdit(
      historyId,
      editSessionId,
      baseSegments,
      editedSegments
    );
  }

  async createTranscriptSnapshot(
    historyId: string,
    reason: TranscriptSnapshotReason,
    segments: TranscriptSegment[]
  ): Promise<TranscriptSnapshotMetadata> {
    return this.ops.createTranscriptSnapshot(historyId, reason, segments);
  }

  async listTranscriptSnapshots(historyId: string): Promise<TranscriptSnapshotMetadata[]> {
    return this.ops.listTranscriptSnapshots(historyId);
  }

  async loadTranscriptSnapshot(
    historyId: string,
    snapshotId: string
  ): Promise<TranscriptSnapshotRecord | null> {
    return this.ops.loadTranscriptSnapshot(historyId, snapshotId);
  }

  async buildTranscriptDiff(
    snapshotSegments: TranscriptSegment[],
    currentSegments: TranscriptSegment[]
  ): Promise<TranscriptDiffResult> {
    return this.ops.buildTranscriptDiff(snapshotSegments, currentSegments);
  }

  async restoreTranscriptDiffRows(
    rows: TranscriptDiffRow[],
    selectedRowIds: Iterable<string>
  ): Promise<TranscriptSegment[]> {
    return this.ops.restoreTranscriptDiffRows(rows, selectedRowIds);
  }

  async updateItemMeta(id: string, updates: Partial<HistoryItem>): Promise<void> {
    await this.ops.updateItemMeta(id, updates);
  }

  async updateProjectAssignments(ids: string[], projectId: string | null): Promise<void> {
    if (ids.length === 0) {
      return;
    }
    await this.ops.updateProjectAssignments(ids, projectId);
  }

  async updateProjectAssignmentsByCurrentProject(
    currentProjectId: string,
    nextProjectId: string | null
  ): Promise<void> {
    await this.ops.reassignProject(currentProjectId, nextProjectId);
  }

  async loadSummary(historyId: string): Promise<HistorySummaryPayload | null> {
    return await this.ops.loadSummary(historyId);
  }

  async saveSummary(historyId: string, summaryPayload: HistorySummaryPayload): Promise<void> {
    try {
      await this.ops.saveSummary(historyId, summaryPayload);
    } catch (error) {
      logger.error('[History] Failed to save summary sidecar:', error);
      throw error;
    }
  }

  async deleteSummary(historyId: string): Promise<void> {
    await this.ops.deleteSummary(historyId);
  }

  async getAudioAbsolutePath(historyId: string): Promise<string | null> {
    return await this.ops.resolveAudioPath(historyId);
  }

  async getAudioUrl(historyId: string): Promise<string | null> {
    const fullPath = await this.getAudioAbsolutePath(historyId);
    return fullPath ? this.ops.convertAudioSrc(fullPath) : null;
  }

  async previewAudioCleanup(
    retentionDays: number | null,
    excludeHistoryId: string | null = null
  ): Promise<HistoryAudioCleanupReport> {
    return this.ops.previewAudioCleanup({ retentionDays, excludeHistoryId });
  }

  async cleanupAudio(
    retentionDays: number | null,
    excludeHistoryId: string | null = null
  ): Promise<HistoryAudioCleanupReport> {
    return this.ops.cleanupAudio({ retentionDays, excludeHistoryId });
  }

  async openHistoryFolder(): Promise<void> {
    await this.ops.openFolder();
  }
}

export function createHistoryService(input?: HistoryServiceInput): HistoryService {
  return new HistoryService(input);
}

export const historyService = createHistoryService();
