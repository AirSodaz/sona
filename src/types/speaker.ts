export type SpeakerKind = 'anonymous' | 'identified';

export interface SpeakerTag {
  id: string;
  label: string;
  kind: SpeakerKind;
  score?: number;
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

export interface SpeakerProcessingConfig {
  speakerSegmentationModelPath?: string;
  speakerEmbeddingModelPath?: string;
  speakerProfiles?: SpeakerProfile[];
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

export function normalizeSpeakerTag(input: unknown): SpeakerTag | undefined {
  if (!input || typeof input !== 'object') {
    return undefined;
  }

  const source = input as Partial<SpeakerTag>;
  if (!isNonEmptyString(source.id) || !isNonEmptyString(source.label)) {
    return undefined;
  }

  const kind: SpeakerKind = source.kind === 'identified' ? 'identified' : 'anonymous';
  const score = typeof source.score === 'number' && Number.isFinite(source.score)
    ? source.score
    : undefined;

  return {
    id: source.id.trim(),
    label: source.label.trim(),
    kind,
    score,
  };
}

export function normalizeSpeakerProfileSample(input: unknown): SpeakerProfileSample | null {
  if (!input || typeof input !== 'object') {
    return null;
  }

  const source = input as Partial<SpeakerProfileSample>;
  if (!isNonEmptyString(source.id) || !isNonEmptyString(source.filePath)) {
    return null;
  }

  return {
    id: source.id.trim(),
    filePath: source.filePath.trim(),
    sourceName: isNonEmptyString(source.sourceName) ? source.sourceName.trim() : 'Sample',
    durationSeconds: typeof source.durationSeconds === 'number' && Number.isFinite(source.durationSeconds)
      ? Math.max(0, source.durationSeconds)
      : 0,
  };
}

export function normalizeSpeakerProfile(input: unknown): SpeakerProfile | null {
  if (!input || typeof input !== 'object') {
    return null;
  }

  const source = input as Partial<SpeakerProfile>;
  if (!isNonEmptyString(source.id)) {
    return null;
  }

  const samples = Array.isArray(source.samples)
    ? source.samples
        .map((sample) => normalizeSpeakerProfileSample(sample))
        .filter((sample): sample is SpeakerProfileSample => !!sample)
    : [];

  return {
    id: source.id.trim(),
    name: isNonEmptyString(source.name) ? source.name.trim() : 'Speaker Profile',
    enabled: source.enabled !== false,
    samples,
  };
}

export function normalizeSpeakerProfiles(input: unknown): SpeakerProfile[] {
  if (!Array.isArray(input)) {
    return [];
  }

  return input
    .map((profile) => normalizeSpeakerProfile(profile))
    .filter((profile): profile is SpeakerProfile => !!profile);
}

export function areSpeakerTagsEqual(
  first: SpeakerTag | null | undefined,
  second: SpeakerTag | null | undefined,
): boolean {
  if (!first && !second) {
    return true;
  }

  if (!first || !second) {
    return false;
  }

  return first.id === second.id
    && first.label === second.label
    && first.kind === second.kind;
}
