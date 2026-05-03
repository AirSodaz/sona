import type { SpeakerAttribution, SpeakerCandidate, SpeakerTag } from '../types/speaker';
import type { TranscriptSegment } from '../types/transcript';
import { buildSpeakerReviewSnapshot as buildSpeakerReviewSnapshotFromRust } from './tauri/speaker';

export type SpeakerReviewStatus = 'pending' | 'auto' | 'reviewed';
export type SpeakerReviewRiskReason =
  | 'suggested'
  | 'anonymous'
  | 'low_confidence'
  | 'medium_confidence'
  | 'auto_identified'
  | 'reviewed';
export type SpeakerReviewFilter =
  | 'pending'
  | 'suggested'
  | 'anonymous'
  | 'identified'
  | 'reviewed'
  | 'all';

export interface SpeakerReviewSegmentPreview {
  id: string;
  start: number;
  end: number;
  text: string;
}

export interface SpeakerReviewGroup {
  groupId: string;
  displayLabel: string;
  anonymousLabel: string;
  state: SpeakerAttribution['state'];
  source: SpeakerAttribution['source'];
  confidence: SpeakerAttribution['confidence'];
  reviewStatus: SpeakerReviewStatus;
  riskReason: SpeakerReviewRiskReason;
  priority: number;
  candidates: SpeakerCandidate[];
  speaker?: SpeakerTag;
  segmentCount: number;
  durationSeconds: number;
  firstSegmentId: string;
  firstStart: number;
  previewSegments: SpeakerReviewSegmentPreview[];
}

export interface SpeakerReviewCounts {
  total: number;
  pending: number;
  suggested: number;
  anonymous: number;
  identified: number;
  reviewed: number;
}

export interface SpeakerReviewFilterOption {
  id: SpeakerReviewFilter;
  labelKey: string;
  countKey: keyof SpeakerReviewCounts;
}

export interface SpeakerReviewSnapshot {
  groups: SpeakerReviewGroup[];
  counts: SpeakerReviewCounts;
  visibleGroups: SpeakerReviewGroup[];
  filterOptions: SpeakerReviewFilterOption[];
}

export async function buildSpeakerReviewSnapshot(
  segments: TranscriptSegment[],
  activeFilter: SpeakerReviewFilter,
): Promise<SpeakerReviewSnapshot> {
  return buildSpeakerReviewSnapshotFromRust(segments, activeFilter);
}
