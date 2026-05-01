import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../projectService', () => ({
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

vi.mock('../historyService', () => ({
  historyService: {
    updateProjectAssignments: vi.fn(),
    updateProjectAssignmentsByCurrentProject: vi.fn(),
  },
}));

import { projectService } from '../projectService';
import { useConfigStore } from '../../stores/configStore';
import { useEffectiveConfigStore } from '../../stores/effectiveConfigStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import {
  buildSpeakerCorrectionProfileSections,
  speakerCorrectionService,
} from '../speakerCorrectionService';

describe('speakerCorrectionService', () => {
  beforeEach(() => {
    resetTranscriptStores();
    vi.clearAllMocks();

    useConfigStore.setState((state) => ({
      ...state,
      config: {
        ...state.config,
        speakerProfiles: [
          { id: 'speaker-1', name: 'Alice', enabled: true, samples: [] },
          { id: 'speaker-2', name: 'Bob', enabled: false, samples: [] },
          { id: 'speaker-3', name: 'Carol', enabled: false, samples: [] },
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
  });

  it('groups current-project speaker profiles ahead of the remaining global profiles', () => {
    const sections = buildSpeakerCorrectionProfileSections(
      useConfigStore.getState().config.speakerProfiles,
      useProjectStore.getState().getActiveProject(),
    );

    expect(sections.primaryProfiles.map((profile) => profile.id)).toEqual(['speaker-1']);
    expect(sections.secondaryProfiles.map((profile) => profile.id)).toEqual([
      'speaker-2',
      'speaker-3',
    ]);
  });

  it('rewrites the entire speaker group, syncs project defaults, and preserves merge compatibility', async () => {
    useTranscriptSessionStore.setState((state) => ({
      ...state,
      sourceHistoryId: 'history-1',
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

    await speakerCorrectionService.assignProfileToSpeakerGroup('anonymous-1', 'speaker-2');

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
    expect(
      useEffectiveConfigStore.getState().config.speakerProfiles?.find((profile) => profile.id === 'speaker-2')
        ?.enabled,
    ).toBe(true);

    useTranscriptSessionStore.getState().mergeSegments('seg-1', 'seg-2');

    expect(useTranscriptSessionStore.getState().segments).toEqual([
      expect.objectContaining({
        id: 'seg-1',
        text: 'Hello world',
        speaker: { id: 'speaker-2', label: 'Bob', kind: 'identified' },
      }),
      expect.objectContaining({
        id: 'seg-3',
        speaker: { id: 'anonymous-2', label: 'Speaker 2', kind: 'anonymous' },
      }),
    ]);
  });
});
