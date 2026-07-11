import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { isHistoryItemDraft } from '../types/history';
import type { TranscriptSegment } from '../types/transcript';
import type {
  TranscriptDiffResult,
  TranscriptDiffRow,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from '../types/transcriptSnapshot';
import { historyService } from './historyService';

export interface TranscriptSnapshotServicePorts {
  useHistoryStore: typeof useHistoryStore;
  useTranscriptSessionStore: typeof useTranscriptSessionStore;
  historyService: typeof historyService;
}

export class TranscriptSnapshotService {
  constructor(private readonly ports: TranscriptSnapshotServicePorts) {}

  private cloneSegments = (segments: TranscriptSegment[]): TranscriptSegment[] => {
    return JSON.parse(JSON.stringify(segments)) as TranscriptSegment[];
  }

  private resolveCurrentSnapshotTarget = (): { historyId: string | null; segments: TranscriptSegment[] } => {
    const session = this.ports.useTranscriptSessionStore.getState();
    return {
      historyId: session.sourceHistoryId,
      segments: session.segments,
    };
  }

  private canSnapshotHistoryId = (historyId: string | null | undefined): historyId is string => {
    return Boolean(historyId && historyId !== 'current');
  }

  private isDraftHistoryItem = (historyId: string): boolean => {
    const item = this.ports.useHistoryStore.getState().items.find((candidate) => candidate.id === historyId);
    return item ? isHistoryItemDraft(item) : false;
  }

  createSnapshot = async (
    historyId: string,
    reason: TranscriptSnapshotReason,
    segments: TranscriptSegment[],
  ): Promise<TranscriptSnapshotMetadata | null> => {
    if (!this.canSnapshotHistoryId(historyId) || this.isDraftHistoryItem(historyId) || segments.length === 0) {
      return null;
    }

    return this.ports.historyService.createTranscriptSnapshot(historyId, reason, this.cloneSegments(segments));
  }

  createSnapshotForCurrentTranscript = async (
    reason: TranscriptSnapshotReason,
  ): Promise<TranscriptSnapshotMetadata | null> => {
    const { historyId, segments } = this.resolveCurrentSnapshotTarget();
    if (!this.canSnapshotHistoryId(historyId)) {
      return null;
    }

    return this.createSnapshot(historyId, reason, segments);
  }

  listSnapshots = async (historyId: string): Promise<TranscriptSnapshotMetadata[]> => {
    if (!this.canSnapshotHistoryId(historyId)) {
      return [];
    }

    return this.ports.historyService.listTranscriptSnapshots(historyId);
  }

  loadSnapshot = async (
    historyId: string,
    snapshotId: string,
  ): Promise<TranscriptSnapshotRecord | null> => {
    if (!this.canSnapshotHistoryId(historyId)) {
      return null;
    }

    return this.ports.historyService.loadTranscriptSnapshot(historyId, snapshotId);
  }

  buildDiff = async (
    snapshotSegments: TranscriptSegment[],
    currentSegments: TranscriptSegment[],
  ): Promise<TranscriptDiffResult> => {
    return this.ports.historyService.buildTranscriptDiff(snapshotSegments, currentSegments);
  }

  restoreDiffRows = async (
    rows: TranscriptDiffRow[],
    selectedRowIds: Iterable<string>,
  ): Promise<TranscriptSegment[]> => {
    return this.ports.historyService.restoreTranscriptDiffRows(rows, selectedRowIds);
  }
}

export function createTranscriptSnapshotService(ports: TranscriptSnapshotServicePorts): TranscriptSnapshotService {
  return new TranscriptSnapshotService(ports);
}

export const transcriptSnapshotService = createTranscriptSnapshotService({
  useHistoryStore,
  useTranscriptSessionStore,
  historyService,
});
