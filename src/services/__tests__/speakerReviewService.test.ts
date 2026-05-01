import { describe, expect, it } from 'vitest';
import {
  buildSpeakerReviewCounts,
  buildSpeakerReviewGroups,
  filterSpeakerReviewGroups,
} from '../speakerReviewService';
import type { TranscriptSegment } from '../../types/transcript';

function createSegment(
  id: string,
  speakerId: string,
  label: string,
  state: 'identified' | 'suggested' | 'anonymous',
  source: 'auto' | 'manual',
  confidence: 'high' | 'medium' | 'low',
  start: number,
  text = label,
): TranscriptSegment {
  return {
    id,
    text,
    start,
    end: start + 4,
    isFinal: true,
    speaker: {
      id: state === 'identified' ? `profile-${speakerId}` : speakerId,
      label,
      kind: state === 'identified' ? 'identified' : 'anonymous',
    },
    speakerAttribution: {
      groupId: speakerId,
      anonymousLabel: label.startsWith('Speaker') ? label : `Speaker ${speakerId}`,
      state,
      source,
      confidence,
      candidates: state === 'suggested'
        ? [{ profileId: 'alice', profileName: 'Alice', score: 0.82, rank: 1 }]
        : [],
    },
  };
}

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

  it('classifies pending, reviewed, and stable automatic groups for the review queue', () => {
    const groups = buildSpeakerReviewGroups([
      createSegment('seg-suggested', 'anonymous-1', 'Speaker 1', 'suggested', 'auto', 'medium', 20),
      createSegment('seg-anonymous', 'anonymous-2', 'Speaker 2', 'anonymous', 'auto', 'low', 10),
      createSegment('seg-auto-identified', 'anonymous-3', 'Alice', 'identified', 'auto', 'high', 30),
      createSegment('seg-manual', 'anonymous-4', 'Bob', 'identified', 'manual', 'high', 40),
    ]);

    expect(groups.map((group) => [group.groupId, group.reviewStatus, group.riskReason])).toEqual([
      ['anonymous-1', 'pending', 'suggested'],
      ['anonymous-2', 'pending', 'anonymous'],
      ['anonymous-3', 'auto', 'auto_identified'],
      ['anonymous-4', 'reviewed', 'reviewed'],
    ]);

    expect(buildSpeakerReviewCounts(groups)).toEqual({
      total: 4,
      pending: 2,
      suggested: 1,
      anonymous: 1,
      identified: 2,
      reviewed: 1,
    });

    expect(filterSpeakerReviewGroups(groups, 'pending').map((group) => group.groupId)).toEqual([
      'anonymous-1',
      'anonymous-2',
    ]);
    expect(filterSpeakerReviewGroups(groups, 'reviewed').map((group) => group.groupId)).toEqual([
      'anonymous-4',
    ]);
    expect(filterSpeakerReviewGroups(groups, 'identified').map((group) => group.groupId)).toEqual([
      'anonymous-3',
      'anonymous-4',
    ]);
  });

  it('keeps the first three ordered segment previews for each speaker group', () => {
    const groups = buildSpeakerReviewGroups([
      createSegment('seg-3', 'anonymous-1', 'Speaker 1', 'anonymous', 'auto', 'low', 12, 'Third line'),
      createSegment('seg-1', 'anonymous-1', 'Speaker 1', 'anonymous', 'auto', 'low', 2, 'First line'),
      createSegment('seg-4', 'anonymous-1', 'Speaker 1', 'anonymous', 'auto', 'low', 20, 'Fourth line'),
      createSegment('seg-2', 'anonymous-1', 'Speaker 1', 'anonymous', 'auto', 'low', 8, 'Second line'),
    ]);

    expect(groups[0].previewSegments).toEqual([
      { id: 'seg-1', start: 2, end: 6, text: 'First line' },
      { id: 'seg-2', start: 8, end: 12, text: 'Second line' },
      { id: 'seg-3', start: 12, end: 16, text: 'Third line' },
    ]);
  });
});
