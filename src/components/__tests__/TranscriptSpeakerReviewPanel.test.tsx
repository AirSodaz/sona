import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptSpeakerReviewPanel } from '../TranscriptSpeakerReviewPanel';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';

const translations: Record<string, string | ((options: Record<string, unknown>) => string)> = {
  'common.close': 'Close',
  'editor.speaker_review_title': 'Speaker Review',
  'editor.speaker_review_description': 'Review speaker groups that need attention before export.',
  'editor.speaker_review_pending_count': ({ count }) => `Needs review ${count}`,
  'editor.speaker_review_reviewed_count': ({ count }) => `Reviewed ${count}`,
  'editor.speaker_review_total_count': ({ count }) => `Total ${count}`,
  'editor.speaker_review_filter_pending': ({ count }) => `Needs review (${count})`,
  'editor.speaker_review_filter_suggested': ({ count }) => `Suggestions (${count})`,
  'editor.speaker_review_filter_anonymous': ({ count }) => `Anonymous (${count})`,
  'editor.speaker_review_filter_identified': ({ count }) => `Identified (${count})`,
  'editor.speaker_review_filter_reviewed': ({ count }) => `Reviewed (${count})`,
  'editor.speaker_review_filter_all': ({ count }) => `All (${count})`,
  'editor.speaker_review_status_pending': 'Needs review',
  'editor.speaker_review_status_reviewed': 'Reviewed',
  'editor.speaker_review_status_auto': 'Auto',
  'editor.speaker_review_state_suggested': 'Suggestion',
  'editor.speaker_review_state_anonymous': 'Anonymous',
  'editor.speaker_review_state_identified': 'Identified',
  'editor.speaker_review_confidence_high': 'High confidence',
  'editor.speaker_review_confidence_medium': 'Medium confidence',
  'editor.speaker_review_confidence_low': 'Low confidence',
  'editor.speaker_review_segments_count': ({ count }) => `${count} segments`,
  'editor.speaker_review_duration': ({ duration }) => `${duration}`,
  'editor.speaker_review_preview_title': 'Preview',
  'editor.speaker_review_candidates': 'Candidates',
  'editor.speaker_review_no_candidates': 'No candidates',
  'editor.speaker_review_jump': 'Jump to first segment',
  'editor.speaker_review_confirm': 'Confirm current label',
  'editor.speaker_review_apply_top_candidate': ({ candidate }) => `Apply ${candidate}`,
  'editor.speaker_review_assign_profile': 'Assign speaker profile',
  'editor.speaker_review_reset': 'Restore anonymous label',
  'editor.speaker_review_empty': 'No speaker groups match this filter.',
  'editor.speaker_correction_show_more': 'Show all speaker profiles',
  'editor.speaker_correction_hide_more': 'Hide more profiles',
  'editor.speaker_correction_failed': 'Failed to update speaker labels for this transcript.',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string; [key: string]: unknown }) => (
      typeof translations[key] === 'function'
        ? translations[key](options || {})
        : translations[key] || options?.defaultValue || key
    ),
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('TranscriptSpeakerReviewPanel', () => {
  beforeEach(() => {
    resetTranscriptStores();
    useConfigStore.getState().setConfig({
      speakerProfiles: [
        { id: 'alice', name: 'Alice', enabled: true, samples: [] },
        { id: 'bob', name: 'Bob', enabled: true, samples: [] },
        { id: 'carol', name: 'Carol', enabled: true, samples: [] },
      ],
    });
    useProjectStore.setState((state) => ({
      ...state,
      projects: [
        {
          id: 'project-1',
          name: 'Project One',
          createdAt: '2026-05-01T00:00:00.000Z',
          updatedAt: '2026-05-01T00:00:00.000Z',
          defaults: {
            enabledSpeakerProfileIds: ['alice', 'bob'],
          },
        } as any,
      ],
      activeProjectId: 'project-1',
    }));
    useTranscriptSessionStore.getState().setSegments([
      {
        id: 'seg-1',
        text: 'Hello there',
        start: 12,
        end: 18,
        isFinal: true,
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'suggested',
          source: 'auto',
          confidence: 'medium',
          candidates: [
            { profileId: 'alice', profileName: 'Alice', score: 0.79, rank: 1 },
            { profileId: 'bob', profileName: 'Bob', score: 0.72, rank: 2 },
          ],
        },
      },
      {
        id: 'seg-1b',
        text: 'Follow up',
        start: 20,
        end: 22,
        isFinal: true,
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'suggested',
          source: 'auto',
          confidence: 'medium',
          candidates: [
            { profileId: 'alice', profileName: 'Alice', score: 0.79, rank: 1 },
          ],
        },
      },
      {
        id: 'seg-2',
        text: 'Unknown voice',
        start: 30,
        end: 34,
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
        id: 'seg-3',
        text: 'Known Alice',
        start: 40,
        end: 44,
        isFinal: true,
        speaker: { id: 'alice', label: 'Alice', kind: 'identified' },
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
        id: 'seg-4',
        text: 'Confirmed Bob',
        start: 50,
        end: 54,
        isFinal: true,
        speaker: { id: 'bob', label: 'Bob', kind: 'identified' },
        speakerAttribution: {
          groupId: 'anonymous-4',
          anonymousLabel: 'Speaker 4',
          state: 'identified',
          source: 'manual',
          confidence: 'high',
          candidates: [],
        },
      },
    ]);
  });

  it('opens on the pending queue with counts, previews, candidates, and profile actions', () => {
    render(<TranscriptSpeakerReviewPanel isOpen onClose={() => undefined} />);

    expect(screen.getByRole('dialog', { name: 'Speaker Review' })).toBeDefined();
    expect(screen.getByText('Needs review 2')).toBeDefined();
    expect(screen.getByText('Reviewed 1')).toBeDefined();
    expect(screen.getByText('Total 4')).toBeDefined();

    expect(screen.getByRole('button', { name: 'Needs review (2)' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Suggestions (1)' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Anonymous (1)' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Identified (2)' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Reviewed (1)' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'All (4)' })).toBeDefined();

    expect(screen.getByText('Speaker 1')).toBeDefined();
    expect(screen.getByText('Speaker 2')).toBeDefined();
    expect(screen.queryByTestId('speaker-review-group-anonymous-3')).toBeNull();
    expect(screen.queryByTestId('speaker-review-group-anonymous-4')).toBeNull();

    const suggestedGroup = screen.getByTestId('speaker-review-group-anonymous-1');
    expect(within(suggestedGroup).getByText('Medium confidence')).toBeDefined();
    expect(within(suggestedGroup).getByText('0:12')).toBeDefined();
    expect(within(suggestedGroup).getByText('Hello there')).toBeDefined();
    expect(within(suggestedGroup).getByText('0:20')).toBeDefined();
    expect(within(suggestedGroup).getByText('Follow up')).toBeDefined();
    expect(within(suggestedGroup).getByText('Alice 0.79')).toBeDefined();
    expect(within(suggestedGroup).getByText('Bob 0.72')).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Apply Alice' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Confirm current label' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Alice' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Bob' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Show all speaker profiles' })).toBeDefined();
  });

  it('switches between queue filters', async () => {
    render(<TranscriptSpeakerReviewPanel isOpen onClose={() => undefined} />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Reviewed (1)' }));
    });

    expect(screen.getByTestId('speaker-review-group-anonymous-4')).toBeDefined();
    expect(screen.queryByTestId('speaker-review-group-anonymous-1')).toBeNull();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'All (4)' }));
    });

    expect(screen.getByTestId('speaker-review-group-anonymous-1')).toBeDefined();
    expect(screen.getByTestId('speaker-review-group-anonymous-2')).toBeDefined();
    expect(screen.getByTestId('speaker-review-group-anonymous-3')).toBeDefined();
    expect(screen.getByTestId('speaker-review-group-anonymous-4')).toBeDefined();
  });

  it('marks a group reviewed and removes it from the default pending queue', async () => {
    render(<TranscriptSpeakerReviewPanel isOpen onClose={() => undefined} />);

    await act(async () => {
      fireEvent.click(
        within(screen.getByTestId('speaker-review-group-anonymous-1'))
          .getByRole('button', { name: 'Confirm current label' }),
      );
    });

    expect(screen.queryByTestId('speaker-review-group-anonymous-1')).toBeNull();
    expect(screen.getByTestId('speaker-review-group-anonymous-2')).toBeDefined();
    expect(useTranscriptSessionStore.getState().segments.find((segment) => segment.id === 'seg-1')?.speakerAttribution)
      .toEqual(expect.objectContaining({
        state: 'anonymous',
        source: 'manual',
      }));
  });

  it('jumps to the first segment and closes the panel', async () => {
    const onClose = vi.fn();
    render(<TranscriptSpeakerReviewPanel isOpen onClose={onClose} />);

    await act(async () => {
      fireEvent.click(
        within(screen.getByTestId('speaker-review-group-anonymous-1'))
          .getByRole('button', { name: 'Jump to first segment' }),
      );
    });

    expect(useTranscriptPlaybackStore.getState().seekRequest?.time).toBe(12);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
