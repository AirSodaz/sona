import { describe, expect, it } from 'vitest';
import { buildSpeakerReviewGroups } from '../speakerReviewService';
import type { TranscriptSegment } from '../../types/transcript';

describe('buildSpeakerReviewGroups', () => {
  it('sorts suggested groups before anonymous and identified groups', () => {
    const segments: TranscriptSegment[] = [
      {
        id: 'seg-identified',
        text: 'Identified',
        start: 30,
        end: 40,
        isFinal: true,
        speaker: { id: 'speaker-1', label: 'Alice', kind: 'identified' },
        speakerAttribution: {
          groupId: 'anonymous-3',
          anonymousLabel: 'Speaker 3',
          state: 'identified',
          source: 'auto',
          confidence: 'high',
          candidates: [],
        },
      },
      {
        id: 'seg-anonymous',
        text: 'Anonymous',
        start: 10,
        end: 20,
        isFinal: true,
        speaker: { id: 'anonymous-2', label: 'Speaker 2', kind: 'anonymous' },
        speakerAttribution: {
          groupId: 'anonymous-2',
          anonymousLabel: 'Speaker 2',
          state: 'anonymous',
          source: 'auto',
          confidence: 'low',
          candidates: [],
        },
      },
      {
        id: 'seg-suggested',
        text: 'Suggested',
        start: 20,
        end: 25,
        isFinal: true,
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'suggested',
          source: 'auto',
          confidence: 'medium',
          candidates: [
            { profileId: 'speaker-1', profileName: 'Alice', score: 0.79, rank: 1 },
          ],
        },
      },
    ];

    const groups = buildSpeakerReviewGroups(segments);

    expect(groups.map((group) => group.groupId)).toEqual([
      'anonymous-1',
      'anonymous-2',
      'anonymous-3',
    ]);
  });

  it('aggregates duration, segment counts, and jump targets per group', () => {
    const groups = buildSpeakerReviewGroups([
      {
        id: 'seg-1',
        text: 'Hello',
        start: 5,
        end: 8,
        isFinal: true,
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'suggested',
          source: 'auto',
          confidence: 'medium',
          candidates: [
            { profileId: 'speaker-1', profileName: 'Alice', score: 0.79, rank: 1 },
          ],
        },
      },
      {
        id: 'seg-2',
        text: 'World',
        start: 8,
        end: 10,
        isFinal: true,
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'suggested',
          source: 'auto',
          confidence: 'medium',
          candidates: [
            { profileId: 'speaker-1', profileName: 'Alice', score: 0.79, rank: 1 },
          ],
        },
      },
    ]);

    expect(groups).toEqual([
      expect.objectContaining({
        groupId: 'anonymous-1',
        displayLabel: 'Speaker 1',
        segmentCount: 2,
        durationSeconds: 5,
        firstSegmentId: 'seg-1',
        firstStart: 5,
      }),
    ]);
  });
});
