import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG } from '../../../stores/configStore';
import type { AppConfig } from '../../../types/config';
import type { ProjectDefaults, ProjectRecord } from '../../../types/project';
import { hydrateAppStartupState } from '../hydration';

const mockMigrateConfig = vi.fn();
const mockProjectMigration = vi.fn();
const mockI18nChangeLanguage = vi.fn();
const mockSettingsGet = vi.fn();
const mockSettingsSet = vi.fn();
const mockSettingsSave = vi.fn();
const mockProjectSaveAll = vi.fn();
const mockSetConfig = vi.fn();
const mockSetCaptionMode = vi.fn();
const mockSetPersistedState = vi.fn();
const mockProjectSetState = vi.fn();

function createProjectRecord(
  overrides: Partial<Omit<ProjectRecord, 'defaults'>> = {},
  defaultOverrides: Partial<ProjectDefaults> = {},
): ProjectRecord {
  return {
    id: 'project-1',
    name: 'Project 1',
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
      enabledSpeakerProfileIds: [],
      ...defaultOverrides,
    },
    ...overrides,
  };
}

const configState = {
  config: { ...DEFAULT_CONFIG },
  setConfig: (patch: Partial<AppConfig>) => {
    configState.config = { ...configState.config, ...patch };
    mockSetConfig(patch);
  },
};

const transcriptRuntimeState = {
  setIsCaptionMode: mockSetCaptionMode,
};

const onboardingState = {
  setPersistedState: mockSetPersistedState,
};

const projectState = {
  projects: [] as ProjectRecord[],
  loadProjects: vi.fn(async () => {
    projectState.projects = [createProjectRecord()];
  }),
};

vi.mock('../../../i18n', () => ({
  default: {
    changeLanguage: (...args: unknown[]) => mockI18nChangeLanguage(...args),
  },
}));

vi.mock('../../configMigrationService', () => ({
  migrateConfig: (...args: unknown[]) => mockMigrateConfig(...args),
}));

vi.mock('../../../types/project', async () => {
  const actual = await vi.importActual<typeof import('../../../types/project')>('../../../types/project');
  return {
    ...actual,
    migrateProjectPolishDefaults: (...args: unknown[]) => mockProjectMigration(...args),
  };
});

vi.mock('../../storageService', () => ({
  settingsStore: {
    get: (...args: unknown[]) => mockSettingsGet(...args),
    set: (...args: unknown[]) => mockSettingsSet(...args),
    save: (...args: unknown[]) => mockSettingsSave(...args),
  },
  STORE_KEY_CONFIG: 'sona-config',
  STORE_KEY_ONBOARDING: 'sona-onboarding',
}));

vi.mock('../../projectService', () => ({
  projectService: {
    saveAll: (...args: unknown[]) => mockProjectSaveAll(...args),
  },
}));

vi.mock('../../../stores/configStore', async () => {
  const actual = await vi.importActual<typeof import('../../../stores/configStore')>('../../../stores/configStore');
  return {
    ...actual,
    useConfigStore: {
      getState: vi.fn(() => configState),
    },
  };
});

vi.mock('../../../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: {
    getState: vi.fn(() => transcriptRuntimeState),
  },
}));

vi.mock('../../../stores/onboardingStore', () => ({
  useOnboardingStore: {
    getState: vi.fn(() => onboardingState),
  },
}));

vi.mock('../../../stores/projectStore', () => ({
  useProjectStore: {
    getState: vi.fn(() => projectState),
    setState: (...args: unknown[]) => mockProjectSetState(...args),
  },
}));

describe('hydrateAppStartupState', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configState.config = { ...DEFAULT_CONFIG };
    projectState.projects = [];
    projectState.loadProjects = vi.fn(async () => {
      projectState.projects = [createProjectRecord()];
    });

    localStorage.clear();
    localStorage.setItem('sona-config', JSON.stringify({ recognitionModelPath: '/legacy/model' }));
    localStorage.setItem('sona-onboarding', JSON.stringify({ version: 1, status: 'deferred' }));
    localStorage.setItem('sona-first-run-completed', 'true');

    mockSettingsGet.mockImplementation(async (key: string) => {
      if (key === 'sona-config') return null;
      if (key === 'sona-onboarding') return null;
      return null;
    });
    mockSettingsSet.mockResolvedValue(undefined);
    mockSettingsSave.mockResolvedValue(undefined);
    mockProjectSaveAll.mockResolvedValue(undefined);
    mockMigrateConfig.mockResolvedValue({
      config: {
        ...DEFAULT_CONFIG,
        appLanguage: 'zh-CN',
        startOnLaunch: true,
        streamingModelPath: '/models/stream.onnx',
        offlineModelPath: '/models/offline.onnx',
        polishCustomPresets: [],
      },
      migrated: true,
    });
    mockProjectMigration.mockReturnValue({
      migrated: true,
      projects: [createProjectRecord({}, { polishPresetId: 'meeting' })],
      customPresets: [{ id: 'project-preset', name: 'Migrated Preset', context: 'Context' }],
    });
  });

  it('hydrates config, onboarding, and project migrations and writes them back before clearing legacy keys', async () => {
    await hydrateAppStartupState();

    expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({
      appLanguage: 'zh-CN',
      streamingModelPath: '/models/stream.onnx',
      offlineModelPath: '/models/offline.onnx',
    }));
    expect(mockSetConfig).toHaveBeenCalledWith({
      polishCustomPresets: [{ id: 'project-preset', name: 'Migrated Preset', context: 'Context' }],
    });
    expect(mockSetCaptionMode).toHaveBeenCalledWith(true);
    expect(mockI18nChangeLanguage).toHaveBeenCalledWith('zh-CN');
    expect(projectState.loadProjects).toHaveBeenCalledTimes(1);
    expect(mockProjectSaveAll).toHaveBeenCalledWith([createProjectRecord({}, { polishPresetId: 'meeting' })]);
    expect(mockProjectSetState).toHaveBeenCalledWith({
      projects: [createProjectRecord({}, { polishPresetId: 'meeting' })],
    });
    expect(mockSetPersistedState).toHaveBeenCalledWith(
      expect.objectContaining({ status: 'deferred' }),
      true,
    );
    expect(mockSettingsSet).toHaveBeenNthCalledWith(
      1,
      'sona-config',
      expect.objectContaining({
        polishCustomPresets: [{ id: 'project-preset', name: 'Migrated Preset', context: 'Context' }],
      }),
    );
    expect(mockSettingsSet).toHaveBeenNthCalledWith(
      2,
      'sona-onboarding',
      expect.objectContaining({ status: 'deferred' }),
    );
    expect(mockSettingsSave).toHaveBeenCalledTimes(1);
    expect(localStorage.getItem('sona-config')).toBeNull();
    expect(localStorage.getItem('sona-onboarding')).toBeNull();
    expect(localStorage.getItem('sona-first-run-completed')).toBeNull();
  });

  it('keeps legacy keys when persisting the hydrated state fails', async () => {
    mockProjectMigration.mockReturnValue({
      migrated: false,
      projects: [],
      customPresets: [],
    });
    mockSettingsSave.mockRejectedValue(new Error('save failed'));

    await hydrateAppStartupState();

    expect(localStorage.getItem('sona-config')).not.toBeNull();
    expect(localStorage.getItem('sona-onboarding')).not.toBeNull();
    expect(localStorage.getItem('sona-first-run-completed')).not.toBeNull();
  });
});
