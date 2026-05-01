import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StoreApi } from 'zustand';
import { createStore } from 'zustand/vanilla';

vi.mock('../../../services/projectService', () => ({
  projectService: {
    getAll: vi.fn(),
    getActiveProjectId: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    setActiveProjectId: vi.fn(),
    reorder: vi.fn(),
  },
}));

vi.mock('../../../services/historyService', () => ({
  historyService: {
    updateProjectAssignments: vi.fn(),
    updateProjectAssignmentsByCurrentProject: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, params?: { defaultValue?: string }) => params?.defaultValue ?? key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../Icons', () => ({
  EditIcon: () => <span data-testid="edit-icon" />,
  TrashIcon: () => <span data-testid="trash-icon" />,
  MergeIcon: () => <span data-testid="merge-icon" />,
}));

vi.mock('../SegmentTimestamp', () => ({
  SegmentTimestamp: ({ start }: { start: number }) => <span>{start}</span>,
}));

import { projectService } from '../../../services/projectService';
import { useConfigStore } from '../../../stores/configStore';
import { useProjectStore } from '../../../stores/projectStore';
import { useTranscriptSessionStore } from '../../../stores/transcriptSessionStore';
import { resetTranscriptStores } from '../../../test-utils/transcriptStoreTestUtils';
import { normalizeTranscriptSegment } from '../../../utils/transcriptTiming';
import { SegmentItem } from '../SegmentItem';
import { TranscriptUIContext, type TranscriptUIState } from '../TranscriptUIContext';

describe('SegmentItem speaker correction', () => {
  let uiStore: StoreApi<TranscriptUIState>;

  beforeEach(() => {
    resetTranscriptStores();
    vi.clearAllMocks();

    uiStore = createStore<TranscriptUIState>(() => ({
      newSegmentIds: new Set(),
      activeSegmentId: null,
      editingSegmentId: null,
      totalSegments: 3,
      aligningSegmentIds: new Set(),
    }));

    useConfigStore.setState((state) => ({
      ...state,
      config: {
        ...state.config,
        speakerProfiles: [
          { id: 'speaker-1', name: 'Alice', enabled: true, samples: [] },
          { id: 'speaker-2', name: 'Bob', enabled: false, samples: [] },
        ],
      },
    }));

    useProjectStore.setState((state) => ({
      ...state,
      activeProjectId: 'project-1',
      projects: [
        {
          id: 'project-1',
          name: 'Project One',
          description: '',
          icon: '',
          createdAt: 1,
          updatedAt: 1,
          defaults: {
            summaryTemplateId: 'general',
            translationLanguage: 'zh',
            polishPresetId: 'general',
            exportFileNamePrefix: '',
            enabledTextReplacementSetIds: [],
            enabledHotwordSetIds: [],
            enabledPolishKeywordSetIds: [],
            enabledSpeakerProfileIds: ['speaker-1'],
          },
        },
      ],
    }));

    vi.mocked(projectService.update).mockImplementation(async (id, updates) => {
      const existing = useProjectStore.getState().projects.find((project) => project.id === id);
      if (!existing) {
        return null;
      }

      return {
        ...existing,
        ...updates,
        defaults: updates.defaults ?? existing.defaults,
      };
    });

    useTranscriptSessionStore.setState((state) => ({
      ...state,
      segments: [
        {
          id: 'seg-1',
          start: 0,
          end: 1,
          text: 'Hello',
          isFinal: true,
          speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        },
        {
          id: 'seg-2',
          start: 1,
          end: 2,
          text: 'world',
          isFinal: true,
          speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        },
        {
          id: 'seg-3',
          start: 2,
          end: 3,
          text: 'Other',
          isFinal: true,
          speaker: { id: 'anonymous-2', label: 'Speaker 2', kind: 'anonymous' },
        },
      ],
    }));
  });

  function renderComponent() {
    return render(
      <TranscriptUIContext.Provider value={uiStore}>
        <SegmentItem
          segment={normalizeTranscriptSegment(useTranscriptSessionStore.getState().segments[0])}
          index={0}
          showSpeakerLabel
          onSeek={vi.fn()}
          onEdit={vi.fn()}
          onSave={vi.fn()}
          onDelete={vi.fn()}
          onMergeWithNext={vi.fn()}
          onAnimationEnd={vi.fn()}
        />
      </TranscriptUIContext.Provider>,
    );
  }

  it('opens the correction menu from the speaker badge and reveals global profiles on demand', () => {
    renderComponent();

    fireEvent.click(screen.getByTestId('speaker-badge-seg-1'));

    expect(screen.getByTestId('speaker-correction-menu-seg-1')).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Alice' })).toBeDefined();
    expect(screen.queryByRole('menuitem', { name: 'Bob' })).toBeNull();

    fireEvent.click(screen.getByTestId('speaker-correction-expand-seg-1'));

    expect(screen.getByRole('menuitem', { name: 'Bob' })).toBeDefined();
  });

  it('updates the full speaker group and project defaults when a global profile is chosen', async () => {
    renderComponent();

    fireEvent.click(screen.getByTestId('speaker-badge-seg-1'));
    fireEvent.click(screen.getByTestId('speaker-correction-expand-seg-1'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Bob' }));

    await waitFor(() => {
      expect(useTranscriptSessionStore.getState().segments).toEqual([
        expect.objectContaining({
          id: 'seg-1',
          speaker: { id: 'speaker-2', label: 'Bob', kind: 'identified' },
        }),
        expect.objectContaining({
          id: 'seg-2',
          speaker: { id: 'speaker-2', label: 'Bob', kind: 'identified' },
        }),
        expect.objectContaining({
          id: 'seg-3',
          speaker: { id: 'anonymous-2', label: 'Speaker 2', kind: 'anonymous' },
        }),
      ]);
      expect(useProjectStore.getState().projects[0].defaults.enabledSpeakerProfileIds).toEqual([
        'speaker-1',
        'speaker-2',
      ]);
    });
  });
});
