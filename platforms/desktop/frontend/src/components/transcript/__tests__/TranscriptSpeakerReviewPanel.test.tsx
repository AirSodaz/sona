import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptSpeakerReviewPanel } from '../TranscriptSpeakerReviewPanel';
import { resetTranscriptStores } from '../../../test-utils/transcriptStoreTestUtils';
import { useConfigStore } from '../../../stores/configStore';
import { useProjectStore } from '../../../stores/projectStore';
import { useTranscriptPlaybackStore } from '../../../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../../../stores/transcriptSessionStore';
import { speakerCorrectionService } from '../../../services/speakerCorrectionService';
import type { SpeakerProfile } from '../../../types/speaker';
import type { TranscriptSegment } from '../../../types/transcript';

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

const tauriMocks = vi.hoisted(() => ({
  invokeTauri: vi.fn(),
}));

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

vi.mock('../../../services/tauri/invoke', () => ({
  invokeTauri: tauriMocks.invokeTauri,
}));

function expectGroupActive(group: HTMLElement, isActive = true): void {
  expect(group.classList.contains('is-active')).toBe(isActive);
}

function resolveSegmentGroupId(segment: TranscriptSegment): string {
  return segment.speakerAttribution?.groupId || segment.speaker?.id || '';
}

function resolveReviewStatus(
  state: string,
  source: string,
  confidence: string,
): 'pending' | 'auto' | 'reviewed' {
  if (source === 'manual') {
    return 'reviewed';
  }
  if (state === 'suggested' || state === 'anonymous' || confidence !== 'high') {
    return 'pending';
  }
  return 'auto';
}

function resolveRiskReason(
  state: string,
  source: string,
  confidence: string,
): 'suggested' | 'anonymous' | 'low_confidence' | 'medium_confidence' | 'auto_identified' | 'reviewed' {
  if (source === 'manual') {
    return 'reviewed';
  }
  if (state === 'suggested') {
    return 'suggested';
  }
  if (state === 'anonymous') {
    return 'anonymous';
  }
  if (confidence === 'low') {
    return 'low_confidence';
  }
  if (confidence === 'medium') {
    return 'medium_confidence';
  }
  return 'auto_identified';
}

function riskPriority(reason: ReturnType<typeof resolveRiskReason>): number {
  return [
    'suggested',
    'anonymous',
    'low_confidence',
    'medium_confidence',
    'auto_identified',
    'reviewed',
  ].indexOf(reason);
}

function buildReviewSnapshot(segments: TranscriptSegment[], activeFilter: string) {
  const groupsById = new Map<string, any>();

  segments.forEach((segment) => {
    const attribution = segment.speakerAttribution;
    if (!attribution) {
      return;
    }

    const existing = groupsById.get(attribution.groupId);
    if (existing) {
      existing.segmentCount += 1;
      existing.durationSeconds += Math.max(0, segment.end - segment.start);
      existing.displayDuration = '8 sec from Rust';
      existing.firstStart = Math.min(existing.firstStart, segment.start);
      existing.previewSegments.push({
        id: segment.id,
        start: segment.start,
        end: segment.end,
        displayStart: `rust-start-${segment.id}`,
        displayDuration: `rust-duration-${segment.id}`,
        text: segment.text,
      });
      if (attribution.source === 'manual') {
        existing.source = 'manual';
      }
      return;
    }

    const reviewStatus = resolveReviewStatus(
      attribution.state,
      attribution.source,
      attribution.confidence,
    );
    const riskReason = resolveRiskReason(
      attribution.state,
      attribution.source,
      attribution.confidence,
    );

    groupsById.set(attribution.groupId, {
      groupId: attribution.groupId,
      displayLabel: segment.speaker?.label || attribution.anonymousLabel,
      anonymousLabel: attribution.anonymousLabel,
      state: attribution.state,
      source: attribution.source,
      confidence: attribution.confidence,
      reviewStatus,
      riskReason,
      priority: riskPriority(riskReason),
      candidates: attribution.candidates.map((candidate) => ({
        ...candidate,
        displayScore: `rust-score-${candidate.profileId}`,
      })),
      speaker: segment.speaker,
      segmentCount: 1,
      durationSeconds: Math.max(0, segment.end - segment.start),
      displayDuration: '8 sec from Rust',
      firstSegmentId: segment.id,
      firstStart: segment.start,
      displayStart: `rust-group-start-${segment.id}`,
      previewSegments: [{
        id: segment.id,
        start: segment.start,
        end: segment.end,
        displayStart: `rust-start-${segment.id}`,
        displayDuration: `rust-duration-${segment.id}`,
        text: segment.text,
      }],
    });
  });

  const groups = [...groupsById.values()]
    .map((group) => {
      const reviewStatus = resolveReviewStatus(group.state, group.source, group.confidence);
      const riskReason = resolveRiskReason(group.state, group.source, group.confidence);
      return {
        ...group,
        reviewStatus,
        riskReason,
        priority: riskPriority(riskReason),
        previewSegments: [...group.previewSegments]
          .sort((left, right) => left.start - right.start)
          .slice(0, 3),
      };
    })
    .sort((left, right) => left.priority - right.priority || left.firstStart - right.firstStart);

  const counts = groups.reduce((accumulator, group) => ({
    total: accumulator.total + 1,
    pending: accumulator.pending + (group.reviewStatus === 'pending' ? 1 : 0),
    suggested: accumulator.suggested + (group.state === 'suggested' ? 1 : 0),
    anonymous: accumulator.anonymous + (group.state === 'anonymous' ? 1 : 0),
    identified: accumulator.identified + (group.state === 'identified' ? 1 : 0),
    reviewed: accumulator.reviewed + (group.reviewStatus === 'reviewed' ? 1 : 0),
  }), {
    total: 0,
    pending: 0,
    suggested: 0,
    anonymous: 0,
    identified: 0,
    reviewed: 0,
  });

  const visibleGroups = groups.filter((group) => {
    switch (activeFilter) {
      case 'pending':
        return group.reviewStatus === 'pending';
      case 'suggested':
        return group.state === 'suggested';
      case 'anonymous':
        return group.state === 'anonymous';
      case 'identified':
        return group.state === 'identified';
      case 'reviewed':
        return group.reviewStatus === 'reviewed';
      case 'all':
      default:
        return true;
    }
  });

  return {
    groups,
    counts,
    visibleGroups,
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

function assignProfileToGroup(
  segments: TranscriptSegment[],
  groupId: string,
  targetProfileId: string,
  speakerProfiles: SpeakerProfile[],
): TranscriptSegment[] {
  const targetProfile = speakerProfiles.find((profile) => profile.id === targetProfileId);
  if (!targetProfile) {
    return segments;
  }

  return segments.map((segment) => {
    if (resolveSegmentGroupId(segment) !== groupId) {
      return segment;
    }

    return {
      ...segment,
      speaker: { id: targetProfile.id, label: targetProfile.name, kind: 'identified' },
      speakerAttribution: {
        groupId,
        anonymousLabel: segment.speakerAttribution?.anonymousLabel || segment.speaker?.label || 'Speaker',
        state: 'identified',
        source: 'manual',
        confidence: 'high',
        candidates: segment.speakerAttribution?.candidates || [],
      },
    } as TranscriptSegment;
  });
}

function resetGroupToAnonymous(segments: TranscriptSegment[], groupId: string): TranscriptSegment[] {
  return segments.map((segment) => {
    if (resolveSegmentGroupId(segment) !== groupId) {
      return segment;
    }

    const anonymousLabel = segment.speakerAttribution?.anonymousLabel || segment.speaker?.label || 'Speaker';
    return {
      ...segment,
      speaker: { id: groupId, label: anonymousLabel, kind: 'anonymous' },
      speakerAttribution: {
        groupId,
        anonymousLabel,
        state: 'anonymous',
        source: 'manual',
        confidence: 'low',
        candidates: segment.speakerAttribution?.candidates || [],
      },
    } as TranscriptSegment;
  });
}

function confirmGroupReview(segments: TranscriptSegment[], groupId: string): TranscriptSegment[] {
  return segments.map((segment) => {
    if (resolveSegmentGroupId(segment) !== groupId) {
      return segment;
    }

    const isIdentified = segment.speaker?.kind === 'identified';
    return {
      ...segment,
      speakerAttribution: {
        groupId,
        anonymousLabel: segment.speakerAttribution?.anonymousLabel || segment.speaker?.label || 'Speaker',
        state: isIdentified ? 'identified' : 'anonymous',
        source: 'manual',
        confidence: isIdentified ? 'high' : 'low',
        candidates: segment.speakerAttribution?.candidates || [],
      },
    } as TranscriptSegment;
  });
}

async function renderReviewPanel(onClose = () => undefined): Promise<void> {
  render(<TranscriptSpeakerReviewPanel isOpen onClose={onClose} />);
  await screen.findByRole('button', { name: 'Needs review (2)' });
}

describe('TranscriptSpeakerReviewPanel', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    tauriMocks.invokeTauri.mockReset();
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

    tauriMocks.invokeTauri.mockImplementation(async (command: string, args?: any) => {
      if (command === 'build_speaker_review_snapshot') {
        return buildReviewSnapshot(args.segments, args.activeFilter);
      }

      if (command === 'apply_speaker_profile_to_group') {
        return {
          segments: assignProfileToGroup(
            args.request.segments,
            args.request.groupId,
            args.request.targetProfileId,
            args.request.speakerProfiles,
          ),
          enabledSpeakerProfileIds: args.request.enabledSpeakerProfileIds,
        };
      }

      if (command === 'reset_speaker_group_to_anonymous') {
        return {
          segments: resetGroupToAnonymous(args.request.segments, args.request.groupId),
        };
      }

      if (command === 'confirm_speaker_group_review') {
        return {
          segments: confirmGroupReview(args.request.segments, args.request.groupId),
        };
      }

      throw new Error(`Unexpected command: ${command}`);
    });
  });

  it('renders inside the shared panel modal shell', async () => {
    await renderReviewPanel();

    const dialog = screen.getByRole('dialog', { name: 'Speaker Review' });
    expect(dialog.classList.contains('panel-modal-shell')).toBe(true);
    expect(dialog.querySelector('.panel-modal-header')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-content')).toBeTruthy();
  });

  it('opens on the pending queue with counts, previews, candidates, and profile actions', async () => {
    await renderReviewPanel();

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
    expectGroupActive(suggestedGroup);
    expect(within(suggestedGroup).getByText('Medium confidence')).toBeDefined();
    expect(within(suggestedGroup).getByText('8 sec from Rust')).toBeDefined();
    expect(within(suggestedGroup).getByText('rust-start-seg-1')).toBeDefined();
    expect(within(suggestedGroup).getByText('Hello there')).toBeDefined();
    expect(within(suggestedGroup).getByText('rust-start-seg-1b')).toBeDefined();
    expect(within(suggestedGroup).getByText('Follow up')).toBeDefined();
    expect(within(suggestedGroup).getByText('Alice rust-score-alice')).toBeDefined();
    expect(within(suggestedGroup).getByText('Bob rust-score-bob')).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Apply Alice' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Confirm current label' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Alice' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Bob' })).toBeDefined();
    expect(within(suggestedGroup).getByRole('button', { name: 'Show all speaker profiles' })).toBeDefined();
  });

  it('switches between queue filters', async () => {
    await renderReviewPanel();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Reviewed (1)' }));
    });

    await waitFor(() => {
      expect(screen.getByTestId('speaker-review-group-anonymous-4')).toBeDefined();
      expect(screen.queryByTestId('speaker-review-group-anonymous-1')).toBeNull();
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'All (4)' }));
    });

    await waitFor(() => {
      expect(screen.getByTestId('speaker-review-group-anonymous-1')).toBeDefined();
      expect(screen.getByTestId('speaker-review-group-anonymous-2')).toBeDefined();
      expect(screen.getByTestId('speaker-review-group-anonymous-3')).toBeDefined();
      expect(screen.getByTestId('speaker-review-group-anonymous-4')).toBeDefined();
    });
  });

  it('marks a group reviewed and removes it from the default pending queue', async () => {
    await renderReviewPanel();

    await act(async () => {
      fireEvent.click(
        within(screen.getByTestId('speaker-review-group-anonymous-1'))
          .getByRole('button', { name: 'Confirm current label' }),
      );
    });

    await waitFor(() => {
      expect(screen.queryByTestId('speaker-review-group-anonymous-1')).toBeNull();
      expect(screen.getByTestId('speaker-review-group-anonymous-2')).toBeDefined();
      expect(useTranscriptSessionStore.getState().segments.find((segment) => segment.id === 'seg-1')?.speakerAttribution)
        .toEqual(expect.objectContaining({
          state: 'anonymous',
          source: 'manual',
        }));
    });
  });

  it('moves the active group through the pending queue with arrow shortcuts', async () => {
    await renderReviewPanel();

    const firstGroup = screen.getByTestId('speaker-review-group-anonymous-1');
    const secondGroup = screen.getByTestId('speaker-review-group-anonymous-2');

    expectGroupActive(firstGroup);
    expectGroupActive(secondGroup, false);

    await act(async () => {
      fireEvent.keyDown(window, { key: 'ArrowDown' });
    });

    await waitFor(() => {
      expectGroupActive(firstGroup, false);
      expectGroupActive(secondGroup);
    });

    await act(async () => {
      fireEvent.keyDown(window, { key: 'ArrowUp' });
    });

    await waitFor(() => {
      expectGroupActive(firstGroup);
      expectGroupActive(secondGroup, false);
    });
  });

  it('confirms the active group with Enter and advances to the next pending group', async () => {
    await renderReviewPanel();
    await waitFor(() => {
      expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-1'));
    });

    await act(async () => {
      fireEvent.keyDown(window, { key: 'Enter' });
    });

    await waitFor(() => {
      expect(screen.queryByTestId('speaker-review-group-anonymous-1')).toBeNull();
      expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-2'));
      expect(useTranscriptSessionStore.getState().segments.find((segment) => segment.id === 'seg-1')?.speakerAttribution)
        .toEqual(expect.objectContaining({
          state: 'anonymous',
          source: 'manual',
        }));
    });
  });

  it('applies the active top candidate with A and advances to the next pending group', async () => {
    await renderReviewPanel();
    await waitFor(() => {
      expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-1'));
    });

    await act(async () => {
      fireEvent.keyDown(window, { key: 'a' });
    });

    await waitFor(() => {
      expect(screen.queryByTestId('speaker-review-group-anonymous-1')).toBeNull();
      expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-2'));
      expect(useTranscriptSessionStore.getState().segments.find((segment) => segment.id === 'seg-1')?.speaker)
        .toEqual(expect.objectContaining({
          id: 'alice',
          label: 'Alice',
          kind: 'identified',
        }));
      expect(useTranscriptSessionStore.getState().segments.find((segment) => segment.id === 'seg-1b')?.speaker)
        .toEqual(expect.objectContaining({
          id: 'alice',
          label: 'Alice',
          kind: 'identified',
        }));
    });
  });

  it('restores the active group with R and advances to the next pending group', async () => {
    await renderReviewPanel();
    await waitFor(() => {
      expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-1'));
    });

    await act(async () => {
      fireEvent.keyDown(window, { key: 'r' });
    });

    await waitFor(() => {
      expect(screen.queryByTestId('speaker-review-group-anonymous-1')).toBeNull();
      expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-2'));
      expect(useTranscriptSessionStore.getState().segments.find((segment) => segment.id === 'seg-1')?.speaker)
        .toEqual(expect.objectContaining({
          id: 'anonymous-1',
          label: 'Speaker 1',
          kind: 'anonymous',
        }));
    });
  });

  it('jumps from the active group with J and closes the panel', async () => {
    const onClose = vi.fn();
    await renderReviewPanel(onClose);

    await act(async () => {
      fireEvent.keyDown(window, { key: 'j' });
    });

    expect(useTranscriptPlaybackStore.getState().seekRequest?.time).toBe(12);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('ignores queue shortcuts from form controls', async () => {
    await renderReviewPanel();

    await act(async () => {
      fireEvent.keyDown(screen.getByRole('button', { name: 'Needs review (2)' }), { key: 'Enter' });
    });

    expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-1'));
    expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-2'), false);
  });

  it('does not submit duplicate shortcut actions while a group is busy', async () => {
    let resolveAction: (() => void) | undefined;
    const confirmSpy = vi
      .spyOn(speakerCorrectionService, 'confirmSpeakerGroupReview')
      .mockImplementation(() => new Promise((resolve) => {
        resolveAction = () => resolve(useTranscriptSessionStore.getState().segments);
      }));

    await renderReviewPanel();
    await waitFor(() => {
      expectGroupActive(screen.getByTestId('speaker-review-group-anonymous-1'));
    });

    await act(async () => {
      fireEvent.keyDown(window, { key: 'Enter' });
      fireEvent.keyDown(window, { key: 'Enter' });
    });

    expect(confirmSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveAction?.();
    });
  });

  it('jumps to the first segment and closes the panel', async () => {
    const onClose = vi.fn();
    await renderReviewPanel(onClose);

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
