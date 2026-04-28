import { beforeEach, describe, expect, it, vi } from 'vitest';
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

  const projectState = {
    loadProjects: vi.fn().mockResolvedValue(undefined),
  };

  const historyStoreState = {
    items: [] as any[],
    loadItems: vi.fn().mockResolvedValue(undefined),
  };

  const transcriptStoreState = {
    isRecording: false,
    sourceHistoryId: null as string | null,
    clearSegments: vi.fn(),
    setAudioFile: vi.fn(),
    setAudioUrl: vi.fn(),
    loadTranscript: vi.fn(),
  };

  const automationStoreState = {
    stopAll: vi.fn().mockResolvedValue(undefined),
    loadAndStart: vi.fn().mockResolvedValue(undefined),
  };

  return {
    automationStoreState,
    batchQueueState: {
      queueItems: [] as Array<{ status: string }>,
    },
    config: initialConfig,
    historyStoreState,
    joinMock: vi.fn(async (...parts: string[]) => parts.join('/')),
    loadAutomationProcessedEntriesMock: vi.fn(),
    loadAutomationRulesMock: vi.fn(),
    historyServiceGetAllMock: vi.fn(),
    historyServiceGetAudioUrlMock: vi.fn(),
    historyServiceLoadSummaryMock: vi.fn(),
    historyServiceLoadTranscriptMock: vi.fn(),
    invokeMock: vi.fn(),
    llmUsageInitMock: vi.fn().mockResolvedValue(undefined),
    migrateConfigMock: vi.fn(),
    mkdirMock: vi.fn().mockResolvedValue(undefined),
    openMock: vi.fn(),
    projectServiceGetAllMock: vi.fn(),
    projectServiceSaveAllMock: vi.fn().mockResolvedValue(undefined),
    projectState,
    readTextFileMock: vi.fn(),
    removeMock: vi.fn().mockResolvedValue(undefined),
    saveAutomationProcessedEntriesMock: vi.fn().mockResolvedValue(undefined),
    saveAutomationRulesMock: vi.fn().mockResolvedValue(undefined),
    saveMock: vi.fn(),
    settingsStoreSaveMock: vi.fn().mockResolvedValue(undefined),
    settingsStoreSetMock: vi.fn().mockResolvedValue(undefined),
    tempDirMock: vi.fn(),
    transcriptStoreState,
    writeTextFileMock: vi.fn().mockResolvedValue(undefined),
    existsMock: vi.fn().mockResolvedValue(false),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: testContext.invokeMock,
}));

vi.mock('@tauri-apps/api/path', () => ({
  join: testContext.joinMock,
  tempDir: testContext.tempDirMock,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: testContext.openMock,
  save: testContext.saveMock,
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  BaseDirectory: { AppLocalData: 3 },
  exists: testContext.existsMock,
  mkdir: testContext.mkdirMock,
  readTextFile: testContext.readTextFileMock,
  remove: testContext.removeMock,
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
    getAll: testContext.historyServiceGetAllMock,
    getAudioUrl: testContext.historyServiceGetAudioUrlMock,
    loadSummary: testContext.historyServiceLoadSummaryMock,
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

function makeManifest(overrides: Partial<PreparedBackupImport['manifest']> = {}) {
  return {
    schemaVersion: 1 as const,
    createdAt: '2026-04-29T00:00:00.000Z',
    appVersion: '0.6.3',
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
    ...overrides,
  };
}

describe('backupService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, 'now').mockReturnValue(123456);
    vi.spyOn(Math, 'random').mockReturnValue(0.123456789);

    testContext.batchQueueState.queueItems = [];
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
    testContext.transcriptStoreState.isRecording = false;
    testContext.transcriptStoreState.sourceHistoryId = null;
    testContext.historyStoreState.items = [];
    testContext.historyStoreState.loadItems.mockResolvedValue(undefined);
    testContext.projectState.loadProjects.mockResolvedValue(undefined);
    testContext.automationStoreState.stopAll.mockResolvedValue(undefined);
    testContext.automationStoreState.loadAndStart.mockResolvedValue(undefined);
    testContext.existsMock.mockResolvedValue(false);
    testContext.tempDirMock.mockResolvedValue('/temp');
  });

  it('exports a light backup manifest and excludes draft history items and temporary state paths', async () => {
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
    testContext.historyServiceGetAllMock.mockResolvedValue([
      {
        id: 'history-1',
        audioPath: 'history-1.webm',
        transcriptPath: 'history-1.json',
        title: 'Saved item',
        previewText: '',
        duration: 12,
        timestamp: 1,
        projectId: null,
        status: 'complete',
      },
      {
        id: 'draft-1',
        audioPath: 'draft-1.webm',
        transcriptPath: 'draft-1.json',
        title: 'Draft item',
        previewText: '',
        duration: 2,
        timestamp: 2,
        projectId: null,
        status: 'draft',
        draftSource: 'live_record',
      },
    ]);
    testContext.historyServiceLoadTranscriptMock.mockResolvedValue([
      { id: 'seg-1', start: 0, end: 1, text: 'hello', isFinal: true },
    ]);
    testContext.historyServiceLoadSummaryMock.mockResolvedValue({
      activeTemplateId: 'general',
    });
    testContext.loadAutomationRulesMock.mockResolvedValue([
      {
        id: 'rule-1',
        name: 'Automation',
        projectId: 'project-1',
        presetId: 'meeting_notes',
        watchDirectory: 'C:\\watch',
        recursive: true,
        enabled: true,
        stageConfig: {
          autoPolish: true,
          autoTranslate: false,
          exportEnabled: false,
        },
        exportConfig: {
          directory: 'C:\\exports',
          format: 'txt',
          mode: 'original',
        },
        createdAt: 1,
        updatedAt: 1,
      },
    ]);
    testContext.loadAutomationProcessedEntriesMock.mockResolvedValue([
      {
        ruleId: 'rule-1',
        filePath: 'C:\\watch\\meeting.wav',
        sourceFingerprint: 'fp-1',
        size: 42,
        mtimeMs: 10,
        status: 'complete',
        processedAt: 20,
      },
    ]);
    testContext.readTextFileMock.mockImplementation(async (path: string, options?: { baseDir?: number }) => {
      if (path === 'analytics/llm-usage.json' && options?.baseDir === 3) {
        return '{"schemaVersion":1}';
      }

      throw new Error(`Unexpected readTextFile call: ${path}`);
    });

    const result = await exportBackup();

    expect(result?.archivePath).toBe('/backups/sona-backup.tar.bz2');
    expect(testContext.invokeMock).toHaveBeenCalledWith('create_tar_bz2', expect.objectContaining({
      archivePath: '/backups/sona-backup.tar.bz2',
    }));

    const historyIndexCall = testContext.writeTextFileMock.mock.calls.find((call) => String(call[0]).endsWith('/history/index.json'));
    expect(historyIndexCall).toBeDefined();
    const stagedHistoryItems = JSON.parse(historyIndexCall?.[1] as string);
    expect(stagedHistoryItems).toHaveLength(1);
    expect(stagedHistoryItems[0].id).toBe('history-1');

    const manifestCall = testContext.writeTextFileMock.mock.calls.find((call) => String(call[0]).endsWith('/manifest.json'));
    expect(manifestCall).toBeDefined();
    const manifest = JSON.parse(manifestCall?.[1] as string);
    expect(manifest.historyMode).toBe('light');
    expect(manifest.counts).toEqual(expect.objectContaining({
      projects: 1,
      historyItems: 1,
      transcriptFiles: 1,
      summaryFiles: 1,
      automationRules: 1,
      automationProcessedEntries: 1,
      analyticsFiles: 1,
    }));

    const writtenPaths = testContext.writeTextFileMock.mock.calls.map((call) => String(call[0]));
    expect(writtenPaths.some((path) => path.includes('recovery'))).toBe(false);
    expect(writtenPaths.some((path) => path.includes('onboarding'))).toBe(false);
    expect(writtenPaths.some((path) => path.includes('sona-active-project-id'))).toBe(false);
    expect(testContext.removeMock).toHaveBeenCalledWith(
      expect.stringContaining('/temp/sona-backup-export-123456-'),
      { recursive: true },
    );
  });

  it('cleans up extracted temp files and aborts before mutation when an import archive is missing a transcript', async () => {
    testContext.openMock.mockResolvedValue('/imports/bad-backup.tar.bz2');
    testContext.invokeMock.mockResolvedValue(undefined);
    testContext.readTextFileMock.mockImplementation(async (path: string) => {
      if (path.endsWith('/manifest.json')) {
        return JSON.stringify(makeManifest());
      }
      if (path.endsWith('/config/sona-config.json')) {
        return JSON.stringify(testContext.config);
      }
      if (path.endsWith('/projects/index.json')) {
        return JSON.stringify([]);
      }
      if (path.endsWith('/history/index.json')) {
        return JSON.stringify([
          {
            id: 'history-1',
            audioPath: 'history-1.webm',
            transcriptPath: 'history-1.json',
            title: 'Transcript only',
            previewText: '',
            duration: 3,
            timestamp: 1,
            projectId: null,
            status: 'complete',
          },
        ]);
      }
      if (path.endsWith('/history/history-1.json')) {
        throw new Error('ENOENT: no such file or directory');
      }

      throw new Error(`Unexpected readTextFile call: ${path}`);
    });

    await expect(prepareImportBackup()).rejects.toThrow('Transcript for history item history-1');

    expect(testContext.settingsStoreSetMock).not.toHaveBeenCalled();
    expect(testContext.projectServiceSaveAllMock).not.toHaveBeenCalled();
    expect(testContext.removeMock).toHaveBeenCalledWith(
      expect.stringContaining('/temp/sona-backup-import-123456-'),
      { recursive: true },
    );
  });

  it('applies an imported backup immediately and refreshes config, workspace, history, and automation state', async () => {
    const prepared: PreparedBackupImport = {
      archivePath: '/imports/backup.tar.bz2',
      extractionDir: '/temp/import-ready',
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
      historyItems: [
        {
          id: 'history-1',
          audioPath: 'history-1.webm',
          transcriptPath: 'history-1.json',
          title: 'Imported history',
          previewText: '',
          duration: 8,
          timestamp: 1,
          projectId: null,
          status: 'complete',
        },
      ],
      transcriptFiles: {
        'history-1.json': [
          { id: 'seg-1', start: 0, end: 1, text: 'hello', isFinal: true },
        ],
      },
      summaryFiles: {
        'history-1': {
          activeTemplateId: 'general',
        },
      },
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
    testContext.transcriptStoreState.sourceHistoryId = 'missing-history';
    testContext.historyStoreState.loadItems.mockImplementation(async () => {
      testContext.historyStoreState.items = [];
    });

    await applyImportBackup(prepared);

    expect(testContext.automationStoreState.stopAll).toHaveBeenCalledTimes(1);
    expect(testContext.settingsStoreSetMock).toHaveBeenCalledWith('sona-config', migratedConfig);
    expect(testContext.settingsStoreSaveMock).toHaveBeenCalledTimes(1);
    expect(testContext.projectServiceSaveAllMock).toHaveBeenCalledWith(prepared.projects);
    expect(testContext.saveAutomationRulesMock).toHaveBeenCalledWith(prepared.automationRules);
    expect(testContext.saveAutomationProcessedEntriesMock).toHaveBeenCalledWith(prepared.automationProcessedEntries);
    expect(testContext.projectState.loadProjects).toHaveBeenCalledTimes(1);
    expect(testContext.historyStoreState.loadItems).toHaveBeenCalledTimes(1);
    expect(testContext.automationStoreState.loadAndStart).toHaveBeenCalledTimes(1);
    expect(testContext.transcriptStoreState.clearSegments).toHaveBeenCalledTimes(1);
    expect(testContext.transcriptStoreState.setAudioFile).toHaveBeenCalledWith(null);
    expect(testContext.transcriptStoreState.setAudioUrl).toHaveBeenCalledWith(null);
    expect(testContext.removeMock).toHaveBeenCalledWith('history', {
      baseDir: 3,
      recursive: true,
    });
    expect(testContext.removeMock).toHaveBeenCalledWith('/temp/import-ready', {
      recursive: true,
    });
    expect(testContext.config).toEqual(migratedConfig);
  });
});
