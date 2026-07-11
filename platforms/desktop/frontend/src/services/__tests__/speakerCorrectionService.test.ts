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

vi.mock('../tauri/invoke', () => ({
  invokeTauri: vi.fn(),
}));

import { projectService } from '../projectService';
import { invokeTauri } from '../tauri/invoke';
import { useConfigStore } from '../../stores/configStore';
import { useEffectiveConfigStore } from '../../stores/effectiveConfigStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import type { TranscriptSegment } from '../../types/transcript';
import {
  buildSpeakerCorrectionProfileSections,
  speakerCorrectionService,
} from '../speakerCorrectionService';

function currentSegments(): TranscriptSegment[] {
  return useTranscriptSessionStore.getState().segments;
}

describe('speakerCorrectionService', () => {
  beforeEach(() => {
    resetTranscriptStores();
    vi.clearAllMocks();

    vi.mocked(invokeTauri).mockImplementation(async (command: string, args?: any) => {
      if (command === 'resolve_effective_config') {
        const globalConfig = args?.globalConfig ?? useConfigStore.getState().config;
        const enabledIds = new Set(args?.project?.defaults?.enabledSpeakerProfileIds ?? []);
        return {
          ...globalConfig,
          speakerProfiles: globalConfig.speakerProfiles?.map((profile: any) => ({
            ...profile,
            enabled: enabledIds.has(profile.id),
          })),
        } as never;
      }

      return { segments: [] } as never;
    });

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

  it('delegates profile assignment to Rust, writes returned segments, and syncs project defaults', async () => {
    const initialSegments: TranscriptSegment[] = [
      {
        id: 'seg-1',
        start: 0,
        end: 1,
        text: 'Hello',
        isFinal: true,
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'anonymous',
          source: 'auto',
          confidence: 'low',
          candidates: [],
        },
      },
    ];
    const rewrittenSegments: TranscriptSegment[] = [
      {
        ...initialSegments[0],
        speaker: { id: 'speaker-2', label: 'Bob', kind: 'identified' },
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'identified',
          source: 'manual',
          confidence: 'high',
          candidates: [],
        },
      },
    ];

    useTranscriptSessionStore.setState((state) => ({
      ...state,
      sourceHistoryId: 'history-1',
      segments: initialSegments,
    }));
    vi.mocked(invokeTauri).mockResolvedValueOnce({
      segments: rewrittenSegments,
      enabledSpeakerProfileIds: ['speaker-1', 'speaker-2'],
    } as never);

    const result = await speakerCorrectionService.assignProfileToSpeakerGroup(
      'anonymous-1',
      'speaker-2',
    );

    expect(invokeTauri).toHaveBeenCalledWith('apply_speaker_profile_to_group', {
      request: {
        segments: initialSegments,
        groupId: 'anonymous-1',
        targetProfileId: 'speaker-2',
        speakerProfiles: [
          { id: 'speaker-1', name: 'Alice', enabled: true, samples: [] },
          { id: 'speaker-2', name: 'Bob', enabled: false, samples: [] },
          { id: 'speaker-3', name: 'Carol', enabled: false, samples: [] },
        ],
        enabledSpeakerProfileIds: ['speaker-1'],
      },
    });
    expect(result).toBe(rewrittenSegments);
    expect(currentSegments()).toEqual([
      expect.objectContaining({
        id: 'seg-1',
        speaker: { id: 'speaker-2', label: 'Bob', kind: 'identified' },
        speakerAttribution: rewrittenSegments[0].speakerAttribution,
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
  });

  it('does not update project defaults when Rust omits enabled speaker ids', async () => {
    const rewrittenSegments: TranscriptSegment[] = [
      {
        id: 'seg-1',
        start: 0,
        end: 1,
        text: 'Hello',
        isFinal: true,
      },
    ];

    vi.mocked(invokeTauri).mockResolvedValueOnce({ segments: rewrittenSegments } as never);

    await speakerCorrectionService.assignProfileToSpeakerGroup('anonymous-1', 'speaker-1');

    expect(useProjectStore.getState().projects[0].defaults.enabledSpeakerProfileIds).toEqual([
      'speaker-1',
    ]);
  });

  it('delegates reset group to anonymous to Rust and writes returned segments', async () => {
    const initialSegments: TranscriptSegment[] = [
      {
        id: 'seg-a',
        start: 0,
        end: 1,
        text: 'Hello',
        isFinal: true,
        speaker: { id: 'speaker-2', label: 'Bob', kind: 'identified' },
      },
    ];
    const rewrittenSegments: TranscriptSegment[] = [
      {
        ...initialSegments[0],
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
      },
    ];

    useTranscriptSessionStore.setState((state) => ({
      ...state,
      segments: initialSegments,
    }));
    vi.mocked(invokeTauri).mockResolvedValueOnce({ segments: rewrittenSegments } as never);

    const result = await speakerCorrectionService.resetGroupToAnonymous('anonymous-1');

    expect(invokeTauri).toHaveBeenCalledWith('reset_speaker_group_to_anonymous', {
      request: {
        segments: initialSegments,
        groupId: 'anonymous-1',
      },
    });
    expect(result).toBe(rewrittenSegments);
    expect(currentSegments()).toEqual([
      expect.objectContaining({
        id: 'seg-a',
        speaker: { id: 'anonymous-1', label: 'Speaker 1', kind: 'anonymous' },
      }),
    ]);
  });

  it('delegates confirm review to Rust and writes returned segments', async () => {
    const initialSegments: TranscriptSegment[] = [
      {
        id: 'seg-a',
        start: 0,
        end: 1,
        text: 'Needs review',
        isFinal: true,
        speaker: { id: 'speaker-1', label: 'Alice', kind: 'identified' },
      },
    ];
    const rewrittenSegments: TranscriptSegment[] = [
      {
        ...initialSegments[0],
        speakerAttribution: {
          groupId: 'anonymous-1',
          anonymousLabel: 'Speaker 1',
          state: 'identified',
          source: 'manual',
          confidence: 'high',
          candidates: [],
        },
      },
    ];

    useTranscriptSessionStore.setState((state) => ({
      ...state,
      segments: initialSegments,
    }));
    vi.mocked(invokeTauri).mockResolvedValueOnce({ segments: rewrittenSegments } as never);

    const result = await speakerCorrectionService.confirmSpeakerGroupReview('anonymous-1');

    expect(invokeTauri).toHaveBeenCalledWith('confirm_speaker_group_review', {
      request: {
        segments: initialSegments,
        groupId: 'anonymous-1',
      },
    });
    expect(result).toBe(rewrittenSegments);
    expect(currentSegments()).toEqual([
      expect.objectContaining({
        id: 'seg-a',
        speakerAttribution: rewrittenSegments[0].speakerAttribution,
      }),
    ]);
  });
});
