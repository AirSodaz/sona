import type {
  SpeakerAttribution,
  SpeakerAttributionConfidence,
  SpeakerAttributionSource,
  SpeakerAttributionState,
  SpeakerCandidate,
  SpeakerKind,
  SpeakerProfile,
  SpeakerProfileReadiness,
  SpeakerProfileSample,
  SpeakerTag,
} from '../../types/speaker';

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function toFiniteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
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

export function normalizeSpeakerCandidate(input: unknown): SpeakerCandidate | null {
  if (!input || typeof input !== 'object') {
    return null;
  }

  const source = input as Partial<SpeakerCandidate>;
  if (!isNonEmptyString(source.profileId) || !isNonEmptyString(source.profileName)) {
    return null;
  }

  const score = toFiniteNumber(source.score);
  const rank = toFiniteNumber(source.rank);
  if (score === null || rank === null) {
    return null;
  }

  return {
    profileId: source.profileId.trim(),
    profileName: source.profileName.trim(),
    score,
    rank: Math.max(1, Math.floor(rank)),
  };
}

export function normalizeSpeakerAttribution(input: unknown): SpeakerAttribution | undefined {
  if (!input || typeof input !== 'object') {
    return undefined;
  }

  const source = input as Partial<SpeakerAttribution>;
  if (!isNonEmptyString(source.groupId) || !isNonEmptyString(source.anonymousLabel)) {
    return undefined;
  }

  let state: SpeakerAttributionState;
  if (source.state === 'identified') {
    state = 'identified';
  } else if (source.state === 'suggested') {
    state = 'suggested';
  } else {
    state = 'anonymous';
  }

  const sourceValue: SpeakerAttributionSource = source.source === 'manual' ? 'manual' : 'auto';

  let confidence: SpeakerAttributionConfidence;
  if (source.confidence === 'high') {
    confidence = 'high';
  } else if (source.confidence === 'medium') {
    confidence = 'medium';
  } else {
    confidence = 'low';
  }
  const candidates = Array.isArray(source.candidates)
    ? source.candidates
        .map((candidate) => normalizeSpeakerCandidate(candidate))
        .filter((candidate): candidate is SpeakerCandidate => !!candidate)
        .slice(0, 3)
    : [];

  return {
    groupId: source.groupId.trim(),
    anonymousLabel: source.anonymousLabel.trim(),
    state,
    source: sourceValue,
    confidence,
    candidates,
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

export function deriveSpeakerProfileReadiness(profile: SpeakerProfile): SpeakerProfileReadiness {
  const usableSamples = profile.samples.filter((sample) => sample.durationSeconds >= 4);
  const usableDurationSeconds = usableSamples
    .reduce((sum, sample) => sum + sample.durationSeconds, 0);

  if (usableSamples.length >= 2 && usableDurationSeconds >= 20) {
    return {
      state: 'ready',
      usableSampleCount: usableSamples.length,
      usableDurationSeconds,
      reasonKey: 'settings.speaker_profile_readiness_ready',
    };
  }

  if (usableSamples.length >= 1 && usableDurationSeconds >= 8) {
    return {
      state: 'limited',
      usableSampleCount: usableSamples.length,
      usableDurationSeconds,
      reasonKey: 'settings.speaker_profile_readiness_limited',
    };
  }

  return {
    state: 'not_ready',
    usableSampleCount: usableSamples.length,
    usableDurationSeconds,
    reasonKey: 'settings.speaker_profile_readiness_not_ready',
  };
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
