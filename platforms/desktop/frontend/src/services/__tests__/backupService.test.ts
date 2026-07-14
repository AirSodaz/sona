import { beforeEach, describe, expect, it, vi } from 'vitest';
import packageJson from '../../../package.json';
import type { PreparedBackupImport } from '../../types/backup';

const testContext = vi.hoisted(() => {
  const initialConfig = {
    appLanguage: 'auto',
    theme: 'auto',
    font: 'system',
    streamingModelPath: '',
    batchModelPath: '',
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
    clearActiveTranscriptSessionMock: vi.fn(),
    openTranscriptSessionMock: vi.fn(),
    historyServiceGetAudioUrlMock: vi.fn(),
    historyServiceLoadTranscriptMock: vi.fn(),
    historyStoreState: {
      error: null as string | null,
      items: [] as any[],
      loadItems: vi.fn().mockResolvedValue(undefined),
    },
    invokeMock: vi.fn(),
    llmUsageReadRawMock: vi.fn(),
    llmUsageReplaceRawMock: vi.fn().mockResolvedValue(undefined),
    loadAutomationProcessedEntriesMock: vi.fn(),
    loadAutomationRulesMock: vi.fn(),
    migrateConfigMock: vi.fn(),
    mkdirMock: vi.fn().mockResolvedValue(undefined),
    openMock: vi.fn(),
    projectServiceGetAllMock: vi.fn(),
    projectServiceSaveAllMock: vi.fn().mockResolvedValue(undefined),
    projectState: {
      error: null as string | null,
      loadProjects: vi.fn().mockResolvedValue(undefined),
    },
    readTextFileMock: vi.fn(),
    saveAutomationProcessedEntriesMock: vi.fn().mockResolvedValue(undefined),
    saveAutomationRulesMock: vi.fn().mockResolvedValue(undefined),
    saveMock: vi.fn(),
    settingsStoreSaveMock: vi.fn().mockResolvedValue(undefined),
    settingsStoreGetMock: vi.fn(),
    settingsStoreNotifyExternalUpdateMock: vi.fn().mockResolvedValue(undefined),
    settingsStoreSetMock: vi.fn().mockResolvedValue(undefined),
    transcriptPlaybackState: {
      setAudioFile: vi.fn(),
    },
    transcriptRuntimeState: {
      isRecording: false,
    },
    transcriptSessionState: {
      sourceHistoryId: null as string | null,
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

vi.mock('../automation/automationService', () => ({
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

vi.mock('../tauri/llmUsage', () => ({
  llmUsageReadRaw: testContext.llmUsageReadRawMock,
  llmUsageReplaceRaw: testContext.llmUsageReplaceRawMock,
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
    get: testContext.settingsStoreGetMock,
    notifyExternalUpdate: testContext.settingsStoreNotifyExternalUpdateMock,
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
  DEFAULT_CONFIG: testContext.config,
  useConfigStore: {
    getState: () => ({
      config: testContext.config,
      setConfig: (patch: typeof testContext.config) => {
        testContext.config = { ...testContext.config, ...patch };
      },
    }),
    setState: (nextState: { config?: typeof testContext.config }) => {
      if (nextState.config) {
        testContext.config = nextState.config;
      }
    },
  },
}));

vi.mock('../../stores/transcriptCoordinator', () => ({
  clearActiveTranscriptSession: testContext.clearActiveTranscriptSessionMock,
  openTranscriptSession: testContext.openTranscriptSessionMock,
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

vi.mock('../../stores/transcriptPlaybackStore', () => ({
  useTranscriptPlaybackStore: {
    getState: () => testContext.transcriptPlaybackState,
  },
}));

vi.mock('../../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: {
    getState: () => testContext.transcriptRuntimeState,
  },
}));

vi.mock('../../stores/transcriptSessionStore', () => ({
  useTranscriptSessionStore: {
    getState: () => testContext.transcriptSessionState,
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
    testContext.transcriptRuntimeState.isRecording = false;
    testContext.transcriptSessionState.sourceHistoryId = null;
    testContext.historyStoreState.items = [];
    testContext.historyStoreState.error = null;
    testContext.historyStoreState.loadItems.mockResolvedValue(undefined);
    testContext.projectState.loadProjects.mockResolvedValue(undefined);
    testContext.projectState.error = null;
    testContext.automationStoreState.stopAll.mockResolvedValue(undefined);
    testContext.automationStoreState.loadAndStart.mockResolvedValue(undefined);
    testContext.llmUsageReadRawMock.mockResolvedValue('{"schemaVersion":1}');
    testContext.llmUsageReplaceRawMock.mockResolvedValue(undefined);
    testContext.config = {
      appLanguage: 'auto',
      theme: 'auto',
      font: 'system',
      streamingModelPath: '',
      batchModelPath: '',
      language: 'auto',
      translationLanguage: 'zh',
      polishKeywordSets: [],
      speakerProfiles: [],
    } as any;
    testContext.settingsStoreGetMock.mockResolvedValue(testContext.config);
  });

  it('exports through Rust with only the archive path and app version', async () => {
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
    testContext.invokeMock.mockResolvedValue(makeManifest());

    const result = await exportBackup();

    expect(result?.archivePath).toBe('/backups/sona-backup.tar.bz2');
    expect(testContext.invokeMock).toHaveBeenCalledWith('export_backup_archive', {
      request: {
        archivePath: '/backups/sona-backup.tar.bz2',
        appVersion: packageJson.version,
      },
    });
    expect(testContext.projectServiceGetAllMock).not.toHaveBeenCalled();
    expect(testContext.loadAutomationRulesMock).not.toHaveBeenCalled();
    expect(testContext.loadAutomationProcessedEntriesMock).not.toHaveBeenCalled();
    expect(testContext.llmUsageReadRawMock).not.toHaveBeenCalled();
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
        batchModelPath: '',
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

  it('prepareImportBackup trusts the Rust-normalized payload without repairing sparse fields', async () => {
    const rustPayload = {
      importId: 'import-sparse',
      archivePath: '/imports/sparse.tar.bz2',
      manifest: makeManifest(),
      config: { theme: 'dark' },
      projects: [{ id: 'project-sparse' }],
      automationRules: [{ id: 'rule-sparse' }],
      automationProcessedEntries: [{ ruleId: 'rule-sparse' }],
      analyticsContent: '{"schemaVersion":1}',
    } as unknown as PreparedBackupImport;

    testContext.invokeMock.mockResolvedValue(rustPayload);

    const prepared = await prepareImportBackup({ archivePath: '/imports/sparse.tar.bz2' });

    expect(prepared).toBe(rustPayload);
    expect(prepared?.projects).toEqual([{ id: 'project-sparse' }]);
    expect(prepared?.automationRules).toEqual([{ id: 'rule-sparse' }]);
    expect(prepared?.automationProcessedEntries).toEqual([{ ruleId: 'rule-sparse' }]);
  });

  it('atomically applies once, reloads config before stores, restarts automation, and disposes', async () => {
    const prepared: PreparedBackupImport = {
      importId: 'import-1',
      archivePath: '/imports/backup.tar.bz2',
      manifest: makeManifest(),
      config: {
        appLanguage: 'zh',
        theme: 'dark',
        font: 'system',
        streamingModelPath: '',
        batchModelPath: '',
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
    testContext.settingsStoreGetMock.mockResolvedValue(migratedConfig);
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
    expect(testContext.settingsStoreSetMock).not.toHaveBeenCalled();
    expect(testContext.settingsStoreSaveMock).not.toHaveBeenCalled();
    expect(testContext.settingsStoreNotifyExternalUpdateMock).toHaveBeenCalledWith(
      'sona-config',
      migratedConfig,
    );
    expect(testContext.projectServiceSaveAllMock).not.toHaveBeenCalled();
    expect(testContext.saveAutomationRulesMock).not.toHaveBeenCalled();
    expect(testContext.saveAutomationProcessedEntriesMock).not.toHaveBeenCalled();
    expect(testContext.llmUsageReplaceRawMock).not.toHaveBeenCalled();
    expect(testContext.invokeMock).toHaveBeenCalledWith('apply_prepared_history_import', {
      importId: 'import-1',
    });
    expect(testContext.projectState.loadProjects).toHaveBeenCalledTimes(1);
    expect(testContext.historyStoreState.loadItems).toHaveBeenCalledTimes(1);
    expect(testContext.automationStoreState.loadAndStart).toHaveBeenCalledTimes(1);
    expect(testContext.settingsStoreGetMock.mock.invocationCallOrder[0]).toBeLessThan(
      testContext.projectState.loadProjects.mock.invocationCallOrder[0],
    );
    expect(testContext.projectState.loadProjects.mock.invocationCallOrder[0]).toBeLessThan(
      testContext.historyStoreState.loadItems.mock.invocationCallOrder[0],
    );
    expect(testContext.historyStoreState.loadItems.mock.invocationCallOrder[0]).toBeLessThan(
      testContext.automationStoreState.loadAndStart.mock.invocationCallOrder[0],
    );
    expect(testContext.transcriptPlaybackState.setAudioFile).toHaveBeenCalledTimes(0);
    expect(testContext.invokeMock).toHaveBeenCalledWith('dispose_prepared_backup_import', {
      importId: 'import-1',
    });
    expect(testContext.config).toEqual(expect.objectContaining(migratedConfig));
  });

  it('disposes and recovers automation when an import is blocked before apply', async () => {
    const prepared = { importId: 'blocked', archivePath: '/backup.tar.bz2' } as PreparedBackupImport;
    testContext.transcriptRuntimeState.isRecording = true;
    testContext.invokeMock.mockResolvedValue(undefined);

    await expect(applyImportBackup(prepared)).rejects.toThrow('Stop Live Record');

    expect(testContext.invokeMock).not.toHaveBeenCalledWith('apply_prepared_history_import', expect.anything());
    expect(testContext.automationStoreState.loadAndStart).not.toHaveBeenCalled();
    expect(testContext.invokeMock).toHaveBeenCalledWith('dispose_prepared_backup_import', {
      importId: 'blocked',
    });
  });

  it('preserves a stop failure while recovery and disposal remain best effort', async () => {
    const prepared = { importId: 'stop-failure', archivePath: '/backup.tar.bz2' } as PreparedBackupImport;
    const stopError = new Error('stop failed');
    testContext.automationStoreState.stopAll.mockRejectedValue(stopError);
    testContext.automationStoreState.loadAndStart.mockRejectedValue(new Error('restart failed'));
    testContext.invokeMock.mockImplementation(async (command: string) => {
      if (command === 'dispose_prepared_backup_import') throw new Error('dispose failed');
      throw new Error(`Unexpected invoke: ${command}`);
    });

    await expect(applyImportBackup(prepared)).rejects.toBe(stopError);

    expect(testContext.automationStoreState.loadAndStart).toHaveBeenCalledTimes(1);
    expect(testContext.invokeMock).toHaveBeenCalledWith('dispose_prepared_backup_import', {
      importId: 'stop-failure',
    });
  });

  it.each([
    ['projects', testContext.projectState],
    ['history', testContext.historyStoreState],
  ])('reports a swallowed %s reload error after commit and clears the stale transcript', async (_name, store) => {
    const prepared = { importId: 'reload-failure', archivePath: '/backup.tar.bz2' } as PreparedBackupImport;
    testContext.transcriptSessionState.sourceHistoryId = 'active-history';
    testContext.invokeMock.mockResolvedValue(undefined);
    store.error = 'reload failed';

    await expect(applyImportBackup(prepared)).rejects.toThrow('reload failed');

    expect(testContext.invokeMock).toHaveBeenCalledWith('apply_prepared_history_import', {
      importId: 'reload-failure',
    });
    expect(testContext.automationStoreState.loadAndStart).toHaveBeenCalledTimes(1);
    expect(testContext.clearActiveTranscriptSessionMock).toHaveBeenCalledWith({ clearAudio: true });
    expect(testContext.invokeMock).toHaveBeenCalledWith('dispose_prepared_backup_import', {
      importId: 'reload-failure',
    });
  });

  it('preserves the reload error when transcript cleanup also fails', async () => {
    const prepared = { importId: 'primary-error', archivePath: '/backup.tar.bz2' } as PreparedBackupImport;
    const primaryError = new Error('config reload failed');
    testContext.settingsStoreGetMock.mockRejectedValue(primaryError);
    testContext.clearActiveTranscriptSessionMock.mockImplementation(() => {
      throw new Error('transcript cleanup failed');
    });
    testContext.invokeMock.mockResolvedValue(undefined);

    await expect(applyImportBackup(prepared)).rejects.toBe(primaryError);

    expect(testContext.invokeMock).toHaveBeenCalledWith('dispose_prepared_backup_import', {
      importId: 'primary-error',
    });
  });

  it('reopens the active transcript after the committed restore is reloaded', async () => {
    const prepared = { importId: 'transcript', archivePath: '/backup.tar.bz2' } as PreparedBackupImport;
    const historyItem = { id: 'active-history', title: 'Restored', icon: 'mic' };
    const segments = [{ id: 'segment-1', text: 'restored' }];
    testContext.transcriptSessionState.sourceHistoryId = historyItem.id;
    testContext.historyStoreState.items = [historyItem];
    testContext.historyServiceLoadTranscriptMock.mockResolvedValue(segments);
    testContext.historyServiceGetAudioUrlMock.mockResolvedValue('asset://restored.wav');
    testContext.invokeMock.mockResolvedValue(undefined);

    await applyImportBackup(prepared);

    expect(testContext.openTranscriptSessionMock).toHaveBeenCalledWith({
      segments,
      sourceHistoryId: historyItem.id,
      title: historyItem.title,
      icon: historyItem.icon,
      audioUrl: 'asset://restored.wav',
    });
    expect(testContext.transcriptPlaybackState.setAudioFile).toHaveBeenCalledWith(null);
  });
});
