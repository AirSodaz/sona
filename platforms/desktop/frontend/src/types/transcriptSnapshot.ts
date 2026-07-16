import type {
  TranscriptDiffStatus,
  TranscriptSnapshotMetadata,
  TranscriptSnapshotReason,
} from '../bindings';
import type { TranscriptSegment } from './transcript';

export type { TranscriptDiffStatus, TranscriptSnapshotMetadata, TranscriptSnapshotReason };

export interface TranscriptSnapshotRecord {
  metadata: TranscriptSnapshotMetadata;
  segments: TranscriptSegment[];
}

export interface TranscriptDiffRow {
  id: string;
  status: TranscriptDiffStatus;
  snapshotSegment?: TranscriptSegment;
  currentSegment?: TranscriptSegment;
  snapshotIndex: number | null;
  currentIndex: number | null;
}

export interface TranscriptDiffResult {
  rows: TranscriptDiffRow[];
  changedCount: number;
}
