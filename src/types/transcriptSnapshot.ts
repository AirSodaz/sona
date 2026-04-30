import type { TranscriptSegment } from './transcript';

export type TranscriptSnapshotReason = 'polish' | 'translate' | 'retranscribe' | 'restore';

export interface TranscriptSnapshotMetadata {
  id: string;
  historyId: string;
  reason: TranscriptSnapshotReason;
  createdAt: number;
  segmentCount: number;
}

export interface TranscriptSnapshotRecord {
  metadata: TranscriptSnapshotMetadata;
  segments: TranscriptSegment[];
}

export type TranscriptDiffStatus = 'unchanged' | 'modified' | 'added' | 'removed';

export interface TranscriptDiffRow {
  id: string;
  status: TranscriptDiffStatus;
  snapshotSegment?: TranscriptSegment;
  currentSegment?: TranscriptSegment;
  snapshotIndex: number | null;
  currentIndex: number | null;
}
