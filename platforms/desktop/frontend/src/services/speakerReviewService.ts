import type { SpeakerAttribution, SpeakerTag } from '../types/speaker';
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
  displayStart: string;
  displayDuration: string;
  text: string;
}

export interface SpeakerReviewCandidate {
  profileId: string;
  profileName: string;
  score: number;
  rank: number;
  displayScore: string;
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
  candidates: SpeakerReviewCandidate[];
  speaker?: SpeakerTag;
  segmentCount: number;
  durationSeconds: number;
  displayDuration: string;
  firstSegmentId: string;
  firstStart: number;
  displayStart: string;
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

export interface SpeakerReviewServicePorts {
  buildSpeakerReviewSnapshotFromRust: typeof buildSpeakerReviewSnapshotFromRust;
}

export class SpeakerReviewService {
  constructor(private readonly ports: SpeakerReviewServicePorts) {}

  buildSpeakerReviewSnapshot = async (
    segments: TranscriptSegment[],
    activeFilter: SpeakerReviewFilter,
  ): Promise<SpeakerReviewSnapshot> => {
    return this.ports.buildSpeakerReviewSnapshotFromRust(segments, activeFilter);
  }
}

export function createSpeakerReviewService(ports: SpeakerReviewServicePorts): SpeakerReviewService {
  return new SpeakerReviewService(ports);
}

export const speakerReviewService = createSpeakerReviewService({
  buildSpeakerReviewSnapshotFromRust,
});

export const {
  buildSpeakerReviewSnapshot,
} = speakerReviewService;
