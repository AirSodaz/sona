import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  buildSpeakerReviewSnapshot,
  type SpeakerReviewSnapshot,
} from '../speakerReviewService';
import type { TranscriptSegment } from '../../types/transcript';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

function createSegment(id: string): TranscriptSegment {
  return {
    id,
    text: 'Hello',
    start: 1,
    end: 3,
    isFinal: true,
    speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
  };
}

function createSnapshot(): SpeakerReviewSnapshot {
  return {
    groups: [
      {
        groupId: 'anonymous-1',
        displayLabel: 'Speaker 1',
        anonymousLabel: 'Speaker 1',
        state: 'anonymous',
        source: 'auto',
        confidence: 'low',
        reviewStatus: 'pending',
        riskReason: 'anonymous',
        priority: 1,
        candidates: [],
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        segmentCount: 1,
        durationSeconds: 2,
        displayDuration: '2s',
        firstSegmentId: 'seg-1',
        firstStart: 1,
        displayStart: '0:01',
        previewSegments: [
          {
            id: 'seg-1',
            start: 1,
            end: 3,
            displayStart: '0:01',
            displayDuration: '2s',
            text: 'Hello',
          },
        ],
      },
    ],
    counts: {
      total: 1,
      pending: 1,
      suggested: 0,
      anonymous: 1,
      identified: 0,
      reviewed: 0,
    },
    visibleGroups: [
      {
        groupId: 'anonymous-1',
        displayLabel: 'Speaker 1',
        anonymousLabel: 'Speaker 1',
        state: 'anonymous',
        source: 'auto',
        confidence: 'low',
        reviewStatus: 'pending',
        riskReason: 'anonymous',
        priority: 1,
        candidates: [],
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        segmentCount: 1,
        durationSeconds: 2,
        displayDuration: '2s',
        firstSegmentId: 'seg-1',
        firstStart: 1,
        displayStart: '0:01',
        previewSegments: [
          {
            id: 'seg-1',
            start: 1,
            end: 3,
            displayStart: '0:01',
            displayDuration: '2s',
            text: 'Hello',
          },
        ],
      },
    ],
    filterOptions: [
      { id: 'pending', labelKey: 'editor.speaker_review_filter_pending', countKey: 'pending' },
      { id: 'suggested', labelKey: 'editor.speaker_review_filter_suggested', countKey: 'suggested' },
      { id: 'anonymous', labelKey: 'editor.speaker_review_filter_anonymous', countKey: 'anonymous' },
      { id: 'identified', labelKey: 'editor.speaker_review_filter_identified', countKey: 'identified' },
      { id: 'reviewed', labelKey: 'editor.speaker_review_filter_reviewed', countKey: 'reviewed' },
      { id: 'all', labelKey: 'editor.speaker_review_filter_all', countKey: 'total' },
    ],
  };
}

describe('buildSpeakerReviewSnapshot', () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
  });

  it('delegates snapshot construction to the Rust command with the active filter', async () => {
    const segments = [createSegment('seg-1')];
    const snapshot = createSnapshot();
    mockedInvoke.mockResolvedValue(snapshot);

    await expect(buildSpeakerReviewSnapshot(segments, 'pending')).resolves.toEqual(snapshot);

    expect(mockedInvoke).toHaveBeenCalledWith('build_speaker_review_snapshot', {
      segments,
      activeFilter: 'pending',
    });
  });
});
