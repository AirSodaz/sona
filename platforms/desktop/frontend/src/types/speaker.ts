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

export {
  areSpeakerTagsEqual,
  deriveSpeakerProfileReadiness,
  normalizeSpeakerAttribution,
  normalizeSpeakerCandidate,
  normalizeSpeakerProfile,
  normalizeSpeakerProfiles,
  normalizeSpeakerProfileSample,
  normalizeSpeakerTag,
} from '../services/speaker/speakerProfileNormalization';
