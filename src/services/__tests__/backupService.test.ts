import { beforeEach, describe, expect, it, vi } from 'vitest';
import packageJson from '../../../package.json';
import type { PreparedBackupImport } from '../../types/backup';

const testContext = vi.hoisted(() => {
  const initialConfig = {
    appLanguage: 'auto',
    theme: 'auto',
    font: 'system',
    streamingModelPath: '',
    offlineModelPath: '',
    language: 'auto',
    translationLanguage: 'zh',
    polishKeywordSets: [],
    speakerProfiles: [],
  } as any;

  return {
    automationStoreState: {
      stopAll: vi.fn().mockResolvedValue(undefined),
      loadAndStart: vi.fn().mockResolvedValue(undefined),
    },
    batchQueueState: {
      queueItems: [] as Array<{ status: string }>,
    },
    config: initialConfig,
    historyServiceGetAudioUrlMock: vi.fn(),
    historyServiceLoadTranscriptMock: vi.fn(),
    historyStoreState: {
      items: [] as any[],
      loadItems: vi.fn().mockResolvedValue(undefined),
    },
    invokeMock: vi.fn(),
    llmUsageInitMock: vi.fn().mockResolvedValue(undefined),
    loadAutomationProcessedEntriesMock: vi.fn(),
    loadAutomationRulesMock: vi.fn(),
    migrateConfigMock: vi.fn(),
    mkdirMock: vi.fn().mockResolvedValue(undefined),
    openMock: vi.fn(),
    projectServiceGetAllMock: vi.fn(),
    projectServiceSaveAllMock: vi.fn().mockResolvedValue(undefined),
    projectState: {
      loadProjects: vi.fn().mockResolvedValue(undefined),
    },
    readTextFileMock: vi.fn(),
    saveAutomationProcessedEntriesMock: vi.fn().mockResolvedValue(undefined),
    saveAutomationRulesMock: vi.fn().mockResolvedValue(undefined),
    saveMock: vi.fn(),
    settingsStoreSaveMock: vi.fn().mockResolvedValue(undefined),
    settingsStoreSetMock: vi.fn().mockResolvedValue(undefined),
    transcriptStoreState: {
      isRecording: false,
      sourceHistoryId: null as string | null,
      clearSegments: vi.fn(),
      setAudioFile: vi.fn(),
      setAudioUrl: vi.fn(),
      loadTranscript: vi.fn(),
    },
    writeTextFileMock: vi.fn().mockResolvedValue(undefined),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: testContext.invokeMock,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: testContext.openMock,
  save: testContext.saveMock,
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  BaseDirectory: { AppLocalData: 3 },
  mkdir: testContext.mkdirMock,
  readTextFile: testContext.readTextFileMock,
  writeTextFile: testContext.writeTextFileMock,
}));

vi.mock('../automationService', () => ({
  loadAutomationProcessedEntries: testContext.loadAutomationProcessedEntriesMock,
  loadAutomationRules: testContext.loadAutomationRulesMock,
  saveAutomationProcessedEntries: testContext.saveAutomationProcessedEntriesMock,
  saveAutomationRules: testContext.saveAutomationRulesMock,
}));

vi.mock('../configMigrationService', () => ({
  migrateConfig: testContext.migrateConfigMock,
}));

vi.mock('../historyService', () => ({
  historyService: {
    getAudioUrl: testContext.historyServiceGetAudioUrlMock,
    loadTranscript: testContext.historyServiceLoadTranscriptMock,
  },
}));

vi.mock('../llmUsageService', () => ({
  llmUsageService: {
    init: testContext.llmUsageInitMock,
  },
}));

vi.mock('../projectService', () => ({
  projectService: {
    getAll: testContext.projectServiceGetAllMock,
    saveAll: testContext.projectServiceSaveAllMock,
  },
}));

vi.mock('../storageService', () => ({
  STORE_KEY_CONFIG: 'sona-config',
  settingsStore: {
    save: testContext.settingsStoreSaveMock,
    set: testContext.settingsStoreSetMock,
  },
}));

vi.mock('../../stores/automationStore', () => ({
  useAutomationStore: {
    getState: () => testContext.automationStoreState,
  },
}));

vi.mock('../../stores/batchQueueStore', () => ({
  useBatchQueueStore: {
    getState: () => testContext.batchQueueState,
  },
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: {
    getState: () => ({
      config: testContext.config,
    }),
    setState: (nextState: { config?: typeof testContext.config }) => {
      if (nextState.config) {
        testContext.config = nextState.config;
      }
    },
  },
}));

vi.mock('../../stores/historyStore', () => ({
  useHistoryStore: {
    getState: () => testContext.historyStoreState,
  },
}));

vi.mock('../../stores/projectStore', () => ({
  useProjectStore: {
    getState: () => testContext.projectState,
  },
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: {
    getState: () => testContext.transcriptStoreState,
  },
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    error: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
  },
}));

import { applyImportBackup, exportBackup, prepareImportBackup } from '../backupService';

function makeManifest() {
  return {
    schemaVersion: 1 as const,
    createdAt: '2026-04-29T00:00:00.000Z',
    appVersion: packageJson.version,
    historyMode: 'light' as const,
    scopes: {
      config: true as const,
      workspace: true as const,
      history: true as const,
      automation: true as const,
      analytics: true as const,
    },
    counts: {
      projects: 1,
      historyItems: 1,
      transcriptFiles: 1,
      summaryFiles: 1,
      automationRules: 1,
      automationProcessedEntries: 1,
      analyticsFiles: 1,
    },
  };
}

describe('backupService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    testContext.batchQueueState.queueItems = [];
    testContext.transcriptStoreState.isRecording = false;
    testContext.transcriptStoreState.sourceHistoryId = null;
    testContext.historyStoreState.items = [];
    testContext.historyStoreState.loadItems.mockResolvedValue(undefined);
    testContext.projectState.loadProjects.mockResolvedValue(undefined);
    testContext.automationStoreState.stopAll.mockResolvedValue(undefined);
    testContext.automationStoreState.loadAndStart.mockResolvedValue(undefined);
    testContext.config = {
      appLanguage: 'auto',
      theme: 'auto',
      font: 'system',
      streamingModelPath: '',
      offlineModelPath: '',
      language: 'auto',
      translationLanguage: 'zh',
      polishKeywordSets: [],
      speakerProfiles: [],
    } as any;
  });

  it('exports through the Rust archive command while keeping TS-owned config/project/automation payload assembly', async () => {
    testContext.saveMock.mockResolvedValue('/backups/sona-backup.tar.bz2');
    testContext.projectServiceGetAllMock.mockResolvedValue([
      {
        id: 'project-1',
        name: 'Workspace',
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
    ]);
    testContext.loadAutomationRulesMock.mockResolvedValue([{ id: 'rule-1', name: 'Automation' }]);
    testContext.loadAutomationProcessedEntriesMock.mockResolvedValue([{ ruleId: 'rule-1', filePath: 'C:\\watch\\meeting.wav' }]);
    testContext.readTextFileMock.mockResolvedValue('{"schemaVersion":1}');
    testContext.invokeMock.mockResolvedValue(makeManifest());

    const result = await exportBackup();

    expect(result?.archivePath).toBe('/backups/sona-backup.tar.bz2');
    expect(testContext.invokeMock).toHaveBeenCalledWith('export_backup_archive', {
      request: expect.objectContaining({
        archivePath: '/backups/sona-backup.tar.bz2',
        appVersion: packageJson.version,
        projects: expect.arrayContaining([expect.objectContaining({ id: 'project-1' })]),
        automationRules: [{ id: 'rule-1', name: 'Automation' }],
        automationProcessedEntries: [{ ruleId: 'rule-1', filePath: 'C:\\watch\\meeting.wav' }],
        analyticsContent: '{"schemaVersion":1}',
      }),
    });
  });

  it('prepareImportBackup now returns a handle plus non-history payloads instead of extracted history files', async () => {
    testContext.openMock.mockResolvedValue('/imports/backup.tar.bz2');
    testContext.invokeMock.mockResolvedValue({
      importId: 'import-1',
      archivePath: '/imports/backup.tar.bz2',
      manifest: makeManifest(),
      config: {
        appLanguage: 'zh',
        theme: 'dark',
        font: 'system',
        streamingModelPath: '',
        offlineModelPath: '',
        language: 'auto',
      },
      projects: [
        {
          id: 'project-1',
          name: 'Imported Project',
          description: '',
          createdAt: 1,
          updatedAt: 2,
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
      automationRules: [
        {
          id: 'rule-1',
          name: 'Imported rule',
          projectId: 'project-1',
          presetId: 'meeting_notes',
          watchDirectory: 'C:\\watch',
          recursive: true,
          enabled: true,
          stageConfig: {},
          exportConfig: {},
          createdAt: 1,
          updatedAt: 2,
        },
      ],
      automationProcessedEntries: [
        {
          ruleId: 'rule-1',
          filePath: 'C:\\watch\\meeting.wav',
          sourceFingerprint: 'fp-1',
          size: 42,
          mtimeMs: 9,
          status: 'complete',
          processedAt: 30,
        },
      ],
      analyticsContent: '{"schemaVersion":1}',
    });

    const prepared = await prepareImportBackup();

    expect(testContext.invokeMock).toHaveBeenCalledWith('prepare_backup_import', {
      archivePath: '/imports/backup.tar.bz2',
    });
    expect(prepared).toEqual(expect.objectContaining({
      importId: 'import-1',
      archivePath: '/imports/backup.tar.bz2',
      manifest: expect.objectContaining({
        counts: expect.objectContaining({
          historyItems: 1,
          transcriptFiles: 1,
          summaryFiles: 1,
        }),
      }),
      analyticsContent: '{"schemaVersion":1}',
    }));
  });

  it('applies a prepared backup through the history-import handle and disposes it afterward', async () => {
    const prepared: PreparedBackupImport = {
      importId: 'import-1',
      archivePath: '/imports/backup.tar.bz2',
      manifest: makeManifest(),
      config: {
        appLanguage: 'zh',
        theme: 'dark',
        font: 'system',
        streamingModelPath: '',
        offlineModelPath: '',
        language: 'auto',
      } as any,
      projects: [
        {
          id: 'project-1',
          name: 'Imported Project',
          description: '',
          createdAt: 1,
          updatedAt: 2,
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
      automationRules: [
        {
          id: 'rule-1',
          name: 'Imported rule',
          projectId: 'project-1',
          presetId: 'meeting_notes',
          watchDirectory: 'C:\\watch',
          recursive: true,
          enabled: true,
          stageConfig: {
            autoPolish: true,
            autoTranslate: false,
            exportEnabled: true,
          },
          exportConfig: {
            directory: 'C:\\exports',
            format: 'txt',
            mode: 'original',
          },
          createdAt: 1,
          updatedAt: 2,
        },
      ],
      automationProcessedEntries: [
        {
          ruleId: 'rule-1',
          filePath: 'C:\\watch\\meeting.wav',
          sourceFingerprint: 'fp-1',
          size: 42,
          mtimeMs: 9,
          status: 'complete',
          processedAt: 30,
        },
      ],
      analyticsContent: '{"schemaVersion":1,"totals":{"callCount":2}}',
    };
    const migratedConfig = {
      ...prepared.config,
      configVersion: 6,
    };

    testContext.migrateConfigMock.mockResolvedValue({
      config: migratedConfig,
      migrated: true,
    });
    testContext.historyStoreState.loadItems.mockImplementation(async () => {
      testContext.historyStoreState.items = [];
    });
    testContext.invokeMock.mockImplementation(async (command: string) => {
      if (command === 'apply_prepared_history_import' || command === 'dispose_prepared_backup_import') {
        return undefined;
      }

      throw new Error(`Unexpected invoke: ${command}`);
    });

    await applyImportBackup(prepared);

    expect(testContext.automationStoreState.stopAll).toHaveBeenCalledTimes(1);
    expect(testContext.settingsStoreSetMock).toHaveBeenCalledWith('sona-config', migratedConfig);
    expect(testContext.settingsStoreSaveMock).toHaveBeenCalledTimes(1);
    expect(testContext.projectServiceSaveAllMock).toHaveBeenCalledWith(prepared.projects);
    expect(testContext.saveAutomationRulesMock).toHaveBeenCalledWith(prepared.automationRules);
    expect(testContext.saveAutomationProcessedEntriesMock).toHaveBeenCalledWith(prepared.automationProcessedEntries);
    expect(testContext.writeTextFileMock).toHaveBeenCalledWith(
      'analytics/llm-usage.json',
      prepared.analyticsContent,
      { baseDir: 3 },
    );
    expect(testContext.invokeMock).toHaveBeenCalledWith('apply_prepared_history_import', {
      importId: 'import-1',
    });
    expect(testContext.projectState.loadProjects).toHaveBeenCalledTimes(1);
    expect(testContext.historyStoreState.loadItems).toHaveBeenCalledTimes(1);
    expect(testContext.automationStoreState.loadAndStart).toHaveBeenCalledTimes(1);
    expect(testContext.transcriptStoreState.clearSegments).toHaveBeenCalledTimes(0);
    expect(testContext.invokeMock).toHaveBeenCalledWith('dispose_prepared_backup_import', {
      importId: 'import-1',
    });
    expect(testContext.config).toEqual(migratedConfig);
  });
});
