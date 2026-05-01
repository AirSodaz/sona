import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptSpeakerReviewPanel } from '../TranscriptSpeakerReviewPanel';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string; [key: string]: unknown }) => (
      options?.defaultValue || key
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
    ]);
  });

  it('shows source, confidence, and candidate details for suggested groups', () => {
    render(<TranscriptSpeakerReviewPanel isOpen onClose={() => undefined} />);

    expect(screen.getByText('Speaker 1')).toBeDefined();
    expect(screen.getByText('1 segments · 6s · suggested')).toBeDefined();
    expect(screen.getByText('Source: Automatic · Confidence: Medium')).toBeDefined();
    expect(screen.getByText('Candidates: Alice (0.79) · Bob (0.72)')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Apply Alice' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Alice' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Bob' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Show all speaker profiles' })).toBeDefined();
  });

  it('jumps to the first segment and closes the panel', async () => {
    const onClose = vi.fn();
    render(<TranscriptSpeakerReviewPanel isOpen onClose={onClose} />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Jump to first segment' }));
    });

    expect(useTranscriptPlaybackStore.getState().seekRequest?.time).toBe(12);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
