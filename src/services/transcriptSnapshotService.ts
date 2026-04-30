import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { isHistoryItemDraft } from '../types/history';
import type { TranscriptSegment } from '../types/transcript';
import type {
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
  TranscriptSnapshotRecord,
} from '../types/transcriptSnapshot';
import { normalizeTranscriptSegments } from '../utils/transcriptTiming';
import { historyService } from './historyService';

interface SnapshotTarget {
  historyId: string | null;
  segments: TranscriptSegment[];
}

function cloneSegments(segments: TranscriptSegment[]): TranscriptSegment[] {
  return normalizeTranscriptSegments(JSON.parse(JSON.stringify(segments)) as TranscriptSegment[]);
}

function resolveCurrentSnapshotTarget(): SnapshotTarget {
  const session = useTranscriptSessionStore.getState();
  return {
    historyId: session.sourceHistoryId,
    segments: session.segments,
  };
}

function canSnapshotHistoryId(historyId: string | null | undefined): historyId is string {
  return Boolean(historyId && historyId !== 'current');
}

function isDraftHistoryItem(historyId: string): boolean {
  const item = useHistoryStore.getState().items.find((candidate) => candidate.id === historyId);
  return item ? isHistoryItemDraft(item) : false;
}

export const transcriptSnapshotService = {
  async createSnapshot(
    historyId: string,
    reason: TranscriptSnapshotReason,
    segments: TranscriptSegment[],
  ): Promise<TranscriptSnapshotMetadata | null> {
    if (!canSnapshotHistoryId(historyId) || isDraftHistoryItem(historyId) || segments.length === 0) {
      return null;
    }

    return historyService.createTranscriptSnapshot(historyId, reason, cloneSegments(segments));
  },

  async createSnapshotForCurrentTranscript(
    reason: TranscriptSnapshotReason,
  ): Promise<TranscriptSnapshotMetadata | null> {
    const { historyId, segments } = resolveCurrentSnapshotTarget();
    if (!canSnapshotHistoryId(historyId)) {
      return null;
    }

    return this.createSnapshot(historyId, reason, segments);
  },

  async listSnapshots(historyId: string): Promise<TranscriptSnapshotMetadata[]> {
    if (!canSnapshotHistoryId(historyId)) {
      return [];
    }

    return historyService.listTranscriptSnapshots(historyId);
  },

  async loadSnapshot(
    historyId: string,
    snapshotId: string,
  ): Promise<TranscriptSnapshotRecord | null> {
    if (!canSnapshotHistoryId(historyId)) {
      return null;
    }

    return historyService.loadTranscriptSnapshot(historyId, snapshotId);
  },
};
