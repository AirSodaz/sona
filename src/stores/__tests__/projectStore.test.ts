import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../configStore';
import { useProjectStore } from '../projectStore';
import { createLlmSettings } from '../../services/llm/state';

vi.mock('../../services/projectService', () => ({
  projectService: {
    getAll: vi.fn(),
    getActiveProjectId: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    setActiveProjectId: vi.fn(),
  },
}));

vi.mock('../../services/historyService', () => ({
  historyService: {
    updateProjectAssignments: vi.fn(),
    updateProjectAssignmentsByCurrentProject: vi.fn(),
  },
}));

describe('projectStore', () => {
  beforeEach(() => {
    useProjectStore.setState({
      projects: [],
      activeProjectId: null,
      isLoading: false,
      error: null,
    });
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        polishKeywordSets: [
          { id: 'kw-1', name: 'Brand', enabled: true, keywords: 'Sona' },
          { id: 'kw-2', name: 'Style', enabled: false, keywords: 'Sentence case' },
        ],
        speakerProfiles: [
          { id: 'speaker-1', name: 'Alice', enabled: true, samples: [] },
          { id: 'speaker-2', name: 'Bob', enabled: false, samples: [] },
        ],
      },
    });
    vi.clearAllMocks();
  });

  it('loads persisted projects and active project id', async () => {
    const { projectService } = await import('../../services/projectService');
    (projectService.getAll as any).mockResolvedValue([
      {
        id: 'project-1',
        name: 'Alpha',
        description: '',
        createdAt: 1,
        updatedAt: 1,
        defaults: {
          summaryTemplateId: 'general',
          translationLanguage: 'zh',
          polishPresetId: 'general',
          exportFileNamePrefix: '',
          enabledTextReplacementSetIds: [],
          enabledHotwordSetIds: [],
          enabledPolishKeywordSetIds: ['kw-1'],
          enabledSpeakerProfileIds: ['speaker-1'],
        },
      },
    ]);
    (projectService.getActiveProjectId as any).mockResolvedValue('project-1');

    await useProjectStore.getState().loadProjects();

    expect(projectService.getAll).toHaveBeenCalledWith({
      fallbackEnabledPolishKeywordSetIds: ['kw-1'],
      fallbackEnabledSpeakerProfileIds: ['speaker-1'],
    });
    expect(useProjectStore.getState().projects).toHaveLength(1);
    expect(useProjectStore.getState().activeProjectId).toBe('project-1');
  });

  it('creates a project from global config defaults when custom defaults are omitted', async () => {
    const { projectService } = await import('../../services/projectService');
    (projectService.create as any).mockImplementation(async ({ name, description, defaults }: any) => ({
      id: 'project-1',
      name,
      description,
      createdAt: 1,
      updatedAt: 1,
      defaults,
    }));

    const project = await useProjectStore.getState().createProject(
      { name: 'Alpha' },
      {
        configVersion: 3,
        appLanguage: 'en',
        theme: 'light',
        font: 'system',
        minimizeToTrayOnExit: true,
        autoCheckUpdates: true,
        liveRecordShortcut: 'Ctrl + Space',
        microphoneId: 'default',
        systemAudioDeviceId: 'default',
        muteDuringRecording: false,
        streamingModelPath: '/models/live',
        offlineModelPath: '/models/offline',
        punctuationModelPath: '',
        vadModelPath: '',
        language: 'auto',
        enableTimeline: false,
        enableITN: true,
        vadBufferSize: 5,
        maxConcurrent: 2,
        llmSettings: createLlmSettings(),
        summaryEnabled: true,
        translationLanguage: 'en',
        polishPresetId: 'lecture',
        polishCustomPresets: [],
        polishKeywordSets: [
          { id: 'kw-1', name: 'Brand', enabled: true, keywords: 'Sona' },
          { id: 'kw-2', name: 'Style', enabled: false, keywords: 'Sentence case' },
        ],
        autoPolish: false,
        autoPolishFrequency: 5,
        voiceTypingEnabled: false,
        voiceTypingShortcut: 'Alt+V',
        voiceTypingMode: 'hold',
        textReplacementSets: [{ id: 'set-1', name: 'Set', enabled: true, ignoreCase: false, rules: [] }],
        hotwordSets: [{ id: 'hot-1', name: 'Hot', enabled: true, rules: [] }],
        speakerProfiles: [
          { id: 'speaker-1', name: 'Alice', enabled: true, samples: [] },
          { id: 'speaker-2', name: 'Bob', enabled: false, samples: [] },
        ],
        hotwords: [],
      },
    );

    expect(project?.defaults.translationLanguage).toBe('en');
    expect(project?.defaults.polishPresetId).toBe('lecture');
    expect(project?.defaults.enabledTextReplacementSetIds).toEqual(['set-1']);
    expect(project?.defaults.enabledHotwordSetIds).toEqual(['hot-1']);
    expect(project?.defaults.enabledPolishKeywordSetIds).toEqual(['kw-1']);
    expect(project?.defaults.enabledSpeakerProfileIds).toEqual(['speaker-1']);
    expect(useProjectStore.getState().projects).toHaveLength(1);
  });

  it('deletes the active project and moves its history back to Inbox', async () => {
    const { projectService } = await import('../../services/projectService');
    const { historyService } = await import('../../services/historyService');

    useProjectStore.setState({
      projects: [
        {
          id: 'project-1',
          name: 'Alpha',
          description: '',
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
            enabledSpeakerProfileIds: [],
          },
        },
      ],
      activeProjectId: 'project-1',
    });

    await useProjectStore.getState().deleteProject('project-1');

    expect(historyService.updateProjectAssignmentsByCurrentProject).toHaveBeenCalledWith('project-1', null);
    expect(projectService.delete).toHaveBeenCalledWith('project-1');
    expect(projectService.setActiveProjectId).toHaveBeenCalledWith(null);
    expect(useProjectStore.getState().activeProjectId).toBeNull();
  });

  it('assigns selected history items to a project', async () => {
    const { historyService } = await import('../../services/historyService');

    await useProjectStore.getState().assignHistoryItems(['hist-1', 'hist-2'], 'project-2');

    expect(historyService.updateProjectAssignments).toHaveBeenCalledWith(['hist-1', 'hist-2'], 'project-2');
  });
});
