export type SpeakerKind = 'anonymous' | 'identified';
export type SpeakerAttributionState = 'identified' | 'suggested' | 'anonymous';
export type SpeakerAttributionSource = 'auto' | 'manual';
export type SpeakerAttributionConfidence = 'high' | 'medium' | 'low';
export type SpeakerProfileReadinessState = 'not_ready' | 'limited' | 'ready';

export interface SpeakerTag {
  id: string;
  label: string;
  kind: SpeakerKind;
  score?: number;
}

export interface SpeakerCandidate {
  profileId: string;
  profileName: string;
  score: number;
  rank: number;
}

export interface SpeakerAttribution {
  groupId: string;
  anonymousLabel: string;
  state: SpeakerAttributionState;
  source: SpeakerAttributionSource;
  confidence: SpeakerAttributionConfidence;
  candidates: SpeakerCandidate[];
}

export interface SpeakerProfileSample {
  id: string;
  filePath: string;
  sourceName: string;
  durationSeconds: number;
}

export interface SpeakerProfile {
  id: string;
  name: string;
  enabled: boolean;
  samples: SpeakerProfileSample[];
}

export interface SpeakerProfileReadiness {
  state: SpeakerProfileReadinessState;
  usableSampleCount: number;
  usableDurationSeconds: number;
  reasonKey: string;
}

export interface SpeakerProcessingConfig {
  speakerSegmentationModelPath?: string;
  speakerEmbeddingModelPath?: string;
  speakerProfiles?: SpeakerProfile[];
}

export interface SpeakerCorrectionProfileSections {
  primaryProfiles: SpeakerProfile[];
  secondaryProfiles: SpeakerProfile[];
}

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
