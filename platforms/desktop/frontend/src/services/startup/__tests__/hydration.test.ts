import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG } from '../../../stores/configStore';
import type { AppConfig } from '../../../types/config';
import { hydrateAppStartupState } from '../hydration';

const mockMigrateConfig = vi.fn();
const mockI18nChangeLanguage = vi.fn();
const mockSettingsGet = vi.fn();
const mockSettingsSet = vi.fn();
const mockSettingsSave = vi.fn();
const mockSetConfig = vi.fn();
const mockSetCaptionMode = vi.fn();
const mockSetPersistedState = vi.fn();
const mockLoadProjects = vi.fn();

const configState = {
  config: { ...DEFAULT_CONFIG },
  setConfig: (patch: Partial<AppConfig>) => {
    configState.config = { ...configState.config, ...patch };
    mockSetConfig(patch);
  },
};

vi.mock('../../../i18n', () => ({
  default: { changeLanguage: (...args: unknown[]) => mockI18nChangeLanguage(...args) },
}));

vi.mock('../../configMigrationService', () => ({
  migrateConfig: (...args: unknown[]) => mockMigrateConfig(...args),
}));

vi.mock('../../storageService', () => ({
  settingsStore: {
    get: (...args: unknown[]) => mockSettingsGet(...args),
    set: (...args: unknown[]) => mockSettingsSet(...args),
    save: (...args: unknown[]) => mockSettingsSave(...args),
  },
  STORE_KEY_CONFIG: 'sona-config',
  STORE_KEY_ONBOARDING: 'sona-onboarding',
}));

vi.mock('../../../stores/configStore', async () => {
  const actual = await vi.importActual<typeof import('../../../stores/configStore')>('../../../stores/configStore');
  return { ...actual, useConfigStore: { getState: vi.fn(() => configState) } };
});

vi.mock('../../../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: { getState: vi.fn(() => ({ setIsCaptionMode: mockSetCaptionMode })) },
}));

vi.mock('../../../stores/onboardingStore', () => ({
  useOnboardingStore: { getState: vi.fn(() => ({ setPersistedState: mockSetPersistedState })) },
}));

vi.mock('../../../stores/projectStore', () => ({
  useProjectStore: { getState: vi.fn(() => ({ loadProjects: mockLoadProjects })) },
}));

describe('hydrateAppStartupState', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    localStorage.setItem('sona-config', JSON.stringify({ recognitionModelPath: '/legacy/model' }));
    localStorage.setItem('sona-onboarding', JSON.stringify({ version: 1, status: 'deferred' }));
    localStorage.setItem('sona-first-run-completed', 'true');
    mockSettingsGet.mockResolvedValue(null);
    mockSettingsSet.mockResolvedValue(undefined);
    mockSettingsSave.mockResolvedValue(undefined);
    mockLoadProjects.mockResolvedValue(undefined);
    mockMigrateConfig.mockResolvedValue({
      config: {
        ...DEFAULT_CONFIG,
        appLanguage: 'zh-CN',
        startOnLaunch: true,
        streamingModelPath: '/models/stream.onnx',
        batchModelPath: '/models/batch.onnx',
      },
      migrated: true,
    });
  });

  it('hydrates global state and loads metadata-only Tags without a defaults write-back', async () => {
    await hydrateAppStartupState();

    expect(mockSetConfig).toHaveBeenCalledWith(expect.objectContaining({ appLanguage: 'zh-CN' }));
    expect(mockSetCaptionMode).toHaveBeenCalledWith(true);
    expect(mockI18nChangeLanguage).toHaveBeenCalledWith('zh');
    expect(mockLoadProjects).toHaveBeenCalledTimes(1);
    expect(mockSetPersistedState).toHaveBeenCalled();
    expect(mockSettingsSet).toHaveBeenCalledWith(
      'sona-config',
      expect.objectContaining({ appLanguage: 'zh-CN' }),
    );
    expect(mockSettingsSave).toHaveBeenCalledTimes(1);
  });
});
