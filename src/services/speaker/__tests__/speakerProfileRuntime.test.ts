import { describe, expect, it } from 'vitest';
import {
  areSpeakerTagsEqual,
  deriveSpeakerProfileReadiness,
  normalizeSpeakerAttribution,
  normalizeSpeakerProfiles,
} from '../speakerProfileRuntime';
import type { SpeakerProfile } from '../../../types/speaker';

function createProfile(samples: number[]): SpeakerProfile {
  return {
    id: 'speaker-1',
    name: 'Alice',
    enabled: true,
    samples: samples.map((durationSeconds, index) => ({
      id: `sample-${index + 1}`,
      filePath: `/samples/${index + 1}.wav`,
      sourceName: `Sample ${index + 1}`,
      durationSeconds,
    })),
  };
}

describe('speakerProfileRuntime', () => {
  it('marks profiles with insufficient usable duration as not ready', () => {
    expect(deriveSpeakerProfileReadiness(createProfile([3.9, 2.5]))).toEqual({
      state: 'not_ready',
      usableSampleCount: 0,
      usableDurationSeconds: 0,
      reasonKey: 'settings.speaker_profile_readiness_not_ready',
    });
  });

  it('marks profiles with one usable sample and enough duration as limited', () => {
    expect(deriveSpeakerProfileReadiness(createProfile([8.2, 2]))).toEqual({
      state: 'limited',
      usableSampleCount: 1,
      usableDurationSeconds: 8.2,
      reasonKey: 'settings.speaker_profile_readiness_limited',
    });
  });

  it('marks profiles with two usable samples and enough duration as ready', () => {
    expect(deriveSpeakerProfileReadiness(createProfile([10, 10.5, 3]))).toEqual({
      state: 'ready',
      usableSampleCount: 2,
      usableDurationSeconds: 20.5,
      reasonKey: 'settings.speaker_profile_readiness_ready',
    });
  });

  it('keeps group identity, anonymous label, and the top three candidates', () => {
    expect(normalizeSpeakerAttribution({
      groupId: 'anonymous-1',
      anonymousLabel: 'Speaker 1',
      state: 'suggested',
      source: 'auto',
      confidence: 'medium',
      candidates: [
        { profileId: 'speaker-a', profileName: 'Alice', score: 0.79, rank: 1 },
        { profileId: 'speaker-b', profileName: 'Bob', score: 0.73, rank: 2 },
        { profileId: 'speaker-c', profileName: 'Carol', score: 0.68, rank: 3 },
        { profileId: 'speaker-d', profileName: 'Dan', score: 0.61, rank: 4 },
      ],
    })).toEqual({
      groupId: 'anonymous-1',
      anonymousLabel: 'Speaker 1',
      state: 'suggested',
      source: 'auto',
      confidence: 'medium',
      candidates: [
        { profileId: 'speaker-a', profileName: 'Alice', score: 0.79, rank: 1 },
        { profileId: 'speaker-b', profileName: 'Bob', score: 0.73, rank: 2 },
        { profileId: 'speaker-c', profileName: 'Carol', score: 0.68, rank: 3 },
      ],
    });
  });

  it('normalizes speaker profiles and filters invalid samples', () => {
    expect(normalizeSpeakerProfiles([
      {
        id: ' speaker-a ',
        name: ' Alice ',
        samples: [
          { id: ' sample-a ', filePath: ' C:/sample.wav ', sourceName: '', durationSeconds: 12 },
          { id: '', filePath: 'C:/missing-id.wav', durationSeconds: 8 },
        ],
      },
      { id: '', name: 'Invalid', samples: [] },
    ])).toEqual([
      {
        id: 'speaker-a',
        name: 'Alice',
        enabled: true,
        samples: [
          {
            id: 'sample-a',
            filePath: 'C:/sample.wav',
            sourceName: 'Sample',
            durationSeconds: 12,
          },
        ],
      },
    ]);
  });

  it('compares speaker tags by stable identity fields and ignores score', () => {
    expect(areSpeakerTagsEqual(
      { id: 'speaker-a', label: 'Alice', kind: 'identified', score: 0.9 },
      { id: 'speaker-a', label: 'Alice', kind: 'identified', score: 0.1 },
    )).toBe(true);

    expect(areSpeakerTagsEqual(
      { id: 'speaker-a', label: 'Alice', kind: 'identified' },
      { id: 'speaker-a', label: 'Speaker 1', kind: 'identified' },
    )).toBe(false);
  });
});
