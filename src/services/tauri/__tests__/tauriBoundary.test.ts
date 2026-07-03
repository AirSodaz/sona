import { beforeEach, describe, expect, it, vi } from 'vitest';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { invoke } from '@tauri-apps/api/core';
import { TauriCommand } from '../commands';
import { TauriEvent, buildRecognizerOutputEvent } from '../events';
import { invokeTauri } from '../invoke';
import {
  getAsrRuntimeMetrics,
  getDiagnosticsCoreSnapshot,
  getModelCatalogSnapshot,
  migrateAppConfig,
  loadAppConfig,
  saveAppConfig,
  getAppSetting,
  setAppSetting,
  openLogFolder,
  resolveEffectiveConfig,
  resolveModelCatalogSelectedIds,
  setLogLevel,
  setMinimizeToTray,
} from '../app';
import { startMicrophoneCapture, stopSystemAudioCapture } from '../audio';
import {
  historyCreateLiveDraft,
  historyCreateTranscriptSnapshot,
  historyListTranscriptSnapshots,
  historyLoadTranscriptSnapshot,
  historyQueryWorkspace,
  historySaveImportedFile,
  historySaveRecording,
  historyUpdateTranscript,
} from '../history';
import { generateLlmText, runTranscriptLlmJob } from '../llm';
import { initRecognizer } from '../recognizer';
import { replaceAutomationRuntimeRules } from '../automation';
import {
  automationLoadRepositoryState,
  automationPersistProcessedEntries,
  automationPersistRepositoryState,
  automationPersistRules,
  automationValidateRuleActivation,
} from '../automationRepository';
import { applyPreparedHistoryImport } from '../backup';
import { getDashboardSnapshot } from '../dashboard';
import { exportTranscriptFile } from '../export';
import {
  llmUsageEnsureStorage,
  llmUsageReadRaw,
  llmUsageReplaceRaw,
} from '../llmUsage';
import {
  projectCreate,
  projectDelete,
  projectGetActiveId,
  projectList,
  projectReorder,
  projectSaveAll,
  projectSetActiveId,
  projectUpdate,
} from '../project';
import {
  recoveryLoadSnapshot,
  recoveryPersistQueueSnapshot,
  recoverySaveSnapshot,
} from '../recovery';
import {
  annotateSpeakerSegmentsFromFile,
  applySpeakerProfileToGroup,
  buildSpeakerReviewSnapshot,
  confirmSpeakerGroupReview,
  resetSpeakerGroupToAnonymous,
} from '../speaker';
import { getAuxWindowState, getMousePosition, injectText, setAuxWindowState } from '../system';
import {
  taskLedgerClearResolved,
  taskLedgerLoadSnapshot,
  taskLedgerPatchTask,
  taskLedgerRemoveTask,
  taskLedgerUpsertTask,
} from '../taskLedger';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('tauri boundary wrappers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('keeps production Tauri command calls behind the tauri boundary wrapper', () => {
    const srcRoot = resolve(process.cwd(), 'src');
    const invokeBoundaryFile = resolve(srcRoot, 'services/tauri/invoke.ts');
    const platformBoundaryRoot = resolve(srcRoot, 'services/tauri/platform');
    const allowedCoreImportFiles = new Set([
      invokeBoundaryFile,
      resolve(platformBoundaryRoot, 'assets.ts'),
    ]);
    const platformCapabilityModules = new Set([
      '@tauri-apps/api/dpi',
      '@tauri-apps/api/event',
      '@tauri-apps/api/path',
      '@tauri-apps/api/webviewWindow',
      '@tauri-apps/api/window',
      '@tauri-apps/plugin-dialog',
      '@tauri-apps/plugin-fs',
    ]);
    const allowedInvokeFiles = new Set([
      resolve(srcRoot, 'services/tauri/invoke.ts'),
    ]);
    const violations: string[] = [];

    function isInsidePlatformBoundary(path: string): boolean {
      return path === platformBoundaryRoot || path.startsWith(`${platformBoundaryRoot}\\`) || path.startsWith(`${platformBoundaryRoot}/`);
    }

    function visit(path: string) {
      const stat = statSync(path);
      if (stat.isDirectory()) {
        for (const entry of readdirSync(path)) {
          if (entry === '__tests__') continue;
          visit(resolve(path, entry));
        }
        return;
      }

      if (!/\.(ts|tsx)$/.test(path) || path.endsWith('.test.ts') || path.endsWith('.test.tsx')) {
        return;
      }

      const source = readFileSync(path, 'utf8');
      const tauriImportPattern = /from\s+['"](@tauri-apps\/(?:api\/(?:core|dpi|event|path|webviewWindow|window)|plugin-(?:dialog|fs)))['"]/g;
      for (const match of source.matchAll(tauriImportPattern)) {
        const moduleName = match[1];
        if (moduleName === '@tauri-apps/api/core') {
          if (!allowedCoreImportFiles.has(path)) {
            violations.push(`${relative(srcRoot, path)} imports ${moduleName} outside the tauri boundary`);
          }
          continue;
        }

        if (platformCapabilityModules.has(moduleName) && !isInsidePlatformBoundary(path)) {
          violations.push(`${relative(srcRoot, path)} imports ${moduleName} outside services/tauri/platform`);
        }
      }

      if (!allowedInvokeFiles.has(path) && /\binvoke\s*(?:<[^>]+>)?\s*\(/.test(source)) {
        violations.push(`${relative(srcRoot, path)} calls invoke() directly`);
      }
    }

    expect(existsSync(srcRoot)).toBe(true);
    visit(srcRoot);

    expect(violations).toEqual([]);
  });

  it('invokeTauri omits the payload argument when none is provided', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(true);

    const result = await invokeTauri(TauriCommand.app.hasActiveDownloads);

    expect(result).toBe(true);
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.hasActiveDownloads);
  });

  it('invokeTauri forwards the payload when provided', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{ kind: 'ok', path: 'C:/models/live' }]);

    const result = await invokeTauri(TauriCommand.app.getPathStatuses, {
      paths: ['C:/models/live'],
    });

    expect(result).toEqual([{ kind: 'ok', path: 'C:/models/live' }]);
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.getPathStatuses, {
      paths: ['C:/models/live'],
    });
  });

  it('app wrappers call the centralized command names', async () => {
    await openLogFolder();
    await setMinimizeToTray(false);
    await setLogLevel('debug');

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.app.openLogFolder);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.app.setMinimizeToTray, {
      enabled: false,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.app.setLogLevel, {
      level: 'debug',
    });
  });

  it('app wrappers expose ASR runtime metrics', async () => {
    const metrics = {
      modelLoad: null,
      liveInference: null,
      batchInference: null,
    };
    vi.mocked(invoke).mockResolvedValueOnce(metrics);

    const result = await getAsrRuntimeMetrics();

    expect(result).toEqual(metrics);
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.getAsrRuntimeMetrics);
  });

  it('app wrappers expose the model catalog snapshot', async () => {
    const snapshot = {
      modelsDir: 'C:/models',
      models: [],
      sections: [],
    };
    vi.mocked(invoke).mockResolvedValueOnce(snapshot);

    const result = await getModelCatalogSnapshot();

    expect(result).toEqual(snapshot);
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.getModelCatalogSnapshot);
  });

  it('app wrappers resolve model catalog selected ids', async () => {
    const paths = {
      streamingModelPath: 'C:/models/live',
      offlineModelPath: 'C:/models/offline',
      speakerSegmentationModelPath: '',
      speakerEmbeddingModelPath: 'C:/models/speaker.onnx',
    };
    const selectedIds = {
      streaming: 'live-model',
      offline: 'offline-model',
      speakerSegmentation: null,
      speakerEmbedding: 'speaker-model',
    };
    vi.mocked(invoke).mockResolvedValueOnce(selectedIds);

    const result = await resolveModelCatalogSelectedIds(paths);

    expect(result).toEqual(selectedIds);
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.resolveModelCatalogSelectedIds, {
      paths,
    });
  });

  it('app wrappers expose the diagnostics core snapshot', async () => {
    const coreSnapshot = {
      scannedAt: '2026-05-03T00:00:00.000Z',
      config: {
        streamingModelPath: 'C:/models/live',
        offlineModelPath: 'C:/models/offline',
        vadModelPath: '',
        punctuationModelPath: '',
        microphoneId: 'default',
      },
      selectedModels: {
        live: { id: 'live', name: 'Live Model' },
        offline: { id: 'offline', name: 'Offline Model' },
      },
      modelRules: {
        live: { requiresVad: false, requiresPunctuation: false },
        offline: { requiresVad: false, requiresPunctuation: false },
      },
      pathStatuses: {
        liveModel: { path: 'C:/models/live', kind: 'directory', error: null },
        offlineModel: { path: 'C:/models/offline', kind: 'directory', error: null },
        vad: null,
        punctuation: null,
      },
      permissionState: 'prompt',
      microphoneProbe: {
        options: [],
        available: false,
      },
      systemAudioProbe: {
        options: [],
        available: false,
      },
      voiceTypingReadiness: {
        state: 'off',
        lastErrorMessage: null,
      },
      runtimeEnvironment: {
        ffmpegPath: 'C:/app/ffmpeg.exe',
        ffmpegExists: true,
        logDirPath: 'C:/app/logs',
      },
      asrRuntimeMetrics: {
        modelLoad: null,
        liveInference: null,
        batchInference: null,
      },
      onboardingReady: true,
      punctuationRequired: false,
    };
    const input = {
      config: {
        streamingModelPath: 'C:/models/live',
        offlineModelPath: 'C:/models/offline',
        vadModelPath: '',
        punctuationModelPath: '',
        microphoneId: 'default',
      },
      permissionState: 'prompt',
      microphoneProbe: {
        options: [],
        available: false,
        source: 'fallback',
      },
      systemAudioProbe: {
        options: [],
        available: false,
        source: 'fallback',
      },
      voiceTypingReadiness: {
        state: 'off',
        shortcutConfigured: false,
        liveModelConfigured: false,
        requiresVad: false,
        vadConfigured: true,
        shortcutRegistration: 'idle',
        warmup: 'idle',
        inputDeviceState: 'off',
        runtimeState: 'off',
        lastErrorSource: null,
        lastErrorMessage: null,
      },
    } satisfies Parameters<typeof getDiagnosticsCoreSnapshot>[0];
    vi.mocked(invoke).mockResolvedValueOnce(coreSnapshot);

    const result = await getDiagnosticsCoreSnapshot(input);

    expect(result).toEqual(coreSnapshot);
    expect(result).not.toHaveProperty('overview');
    expect(result).not.toHaveProperty('sections');
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.getDiagnosticsCoreSnapshot, {
      input,
    });
  });

  it('app wrappers delegate config migration and effective config resolution to Rust', async () => {
    const globalConfig = {
      configVersion: 6,
      streamingModelPath: '',
      offlineModelPath: '',
      appLanguage: 'auto',
      language: 'auto',
      llmSettings: {},
      translationLanguage: 'zh',
      polishPresetId: 'general',
    } as any;
    const project = {
      id: 'project-1',
      name: 'Project',
      description: '',
      createdAt: 1,
      updatedAt: 1,
      defaults: {
        summaryTemplateId: 'general',
        translationLanguage: 'ja',
        polishPresetId: 'meeting',
        exportFileNamePrefix: '',
        enabledTextReplacementSetIds: [],
        enabledHotwordSetIds: [],
        enabledPolishKeywordSetIds: [],
        enabledSpeakerProfileIds: [],
      },
    };
    const migrationResult = { config: globalConfig, migrated: false };
    const effectiveConfig = {
      ...globalConfig,
      translationLanguage: 'ja',
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce(migrationResult)
      .mockResolvedValueOnce(effectiveConfig);

    await expect(migrateAppConfig(globalConfig, null, 'Default Rules')).resolves.toEqual(migrationResult);
    await expect(resolveEffectiveConfig(globalConfig, project)).resolves.toEqual(effectiveConfig);

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.app.migrateAppConfig, {
      savedConfig: globalConfig,
      legacyConfig: null,
      defaultRuleSetName: 'Default Rules',
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.app.resolveEffectiveConfig, {
      globalConfig,
      project,
    });
  });

  it('app wrappers expose SQLite-backed config and setting storage', async () => {
    const globalConfig = {
      configVersion: 7,
      appLanguage: 'auto',
      language: 'auto',
      llmSettings: {},
      translationLanguage: 'zh',
      polishPresetId: 'general',
    } as any;
    const onboarding = { version: 1, status: 'completed' };
    vi.mocked(invoke)
      .mockResolvedValueOnce(globalConfig)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(onboarding)
      .mockResolvedValueOnce(undefined);

    await expect(loadAppConfig()).resolves.toEqual(globalConfig);
    await expect(saveAppConfig(globalConfig)).resolves.toBeUndefined();
    await expect(getAppSetting<typeof onboarding>('sona-onboarding')).resolves.toEqual(onboarding);
    await expect(setAppSetting('sona-onboarding', onboarding)).resolves.toBeUndefined();

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.app.loadAppConfig);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.app.saveAppConfig, {
      config: globalConfig,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.app.getAppSetting, {
      key: 'sona-onboarding',
    });
    expect(invoke).toHaveBeenNthCalledWith(4, TauriCommand.app.setAppSetting, {
      key: 'sona-onboarding',
      value: onboarding,
    });
  });

  it('audio wrappers adapt capture arguments and return values', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce('record.wav');

    await startMicrophoneCapture({
      deviceName: 'Mic 1',
      instanceId: 'voice-typing',
      outputPath: 'C:/temp/voice.wav',
    });
    const savedPath = await stopSystemAudioCapture('record');

    expect(savedPath).toBe('record.wav');
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.audio.startMicrophoneCapture, {
      deviceName: 'Mic 1',
      instanceId: 'voice-typing',
      outputPath: 'C:/temp/voice.wav',
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.audio.stopSystemAudioCapture, {
      instanceId: 'record',
    });
  });

  it('history wrappers forward transcript persistence payloads', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ id: 'history-1' });

    const result = await historyUpdateTranscript('history-1', []);

    expect(result).toEqual({ id: 'history-1' });
    expect(invoke).toHaveBeenCalledWith(TauriCommand.history.updateTranscript, {
      historyId: 'history-1',
      segments: [],
    });
  });

  it('history wrappers forward item creation intents without caller-built items', async () => {
    await historyCreateLiveDraft(null, 'webm', 'project-1', 'system:mic');
    await historySaveRecording({
      segments: [],
      duration: 3,
      projectId: 'project-1',
      audioBytes: [1, 2, 3],
      audioExtension: 'webm',
    });
    await historySaveImportedFile({
      sourcePath: 'D:/audio/meeting.mp3',
      segments: [],
      duration: 4,
      projectId: null,
      convertedSourcePath: 'C:/Temp/meeting.wav',
    });

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.history.createLiveDraft, {
      id: null,
      audioExtension: 'webm',
      projectId: 'project-1',
      icon: 'system:mic',
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.history.saveRecording, {
      segments: [],
      duration: 3,
      projectId: 'project-1',
      audioBytes: [1, 2, 3],
      audioExtension: 'webm',
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.history.saveImportedFile, {
      sourcePath: 'D:/audio/meeting.mp3',
      segments: [],
      duration: 4,
      projectId: null,
      convertedSourcePath: 'C:/Temp/meeting.wav',
    });
  });

  it('history wrappers forward transcript snapshot payloads', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ id: 'snapshot-1' })
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(null);

    await historyCreateTranscriptSnapshot('history-1', 'polish', []);
    await historyListTranscriptSnapshots('history-1');
    await historyLoadTranscriptSnapshot('history-1', 'snapshot-1');

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.history.createTranscriptSnapshot, {
      historyId: 'history-1',
      reason: 'polish',
      segments: [],
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.history.listTranscriptSnapshots, {
      historyId: 'history-1',
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.history.loadTranscriptSnapshot, {
      historyId: 'history-1',
      snapshotId: 'snapshot-1',
    });
  });

  it('history workspace query wrapper forwards flat query args', async () => {
    await historyQueryWorkspace({
      scope: { kind: 'project', projectId: 'project-1' },
      query: 'roadmap',
      filterType: 'recording',
      dateFilter: 'week',
      sortOrder: 'title_asc',
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.history.queryWorkspace, {
      scope: { kind: 'project', projectId: 'project-1' },
      query: 'roadmap',
      filterType: 'recording',
      dateFilter: 'week',
      sortOrder: 'title_asc',
    });
  });

  it('dashboard snapshot wrapper wraps request args under request', async () => {
    await getDashboardSnapshot({ deep: true });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.dashboard.getSnapshot, {
      request: { deep: true },
    });
  });

  it('export transcript wrapper forwards flat export args', async () => {
    await exportTranscriptFile({
      segments: [],
      format: 'srt',
      mode: 'bilingual',
      outputPath: 'C:/exports/transcript.srt',
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.export.transcriptFile, {
      segments: [],
      format: 'srt',
      mode: 'bilingual',
      outputPath: 'C:/exports/transcript.srt',
    });
  });

  it('llm wrappers wrap requests under the request key', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('generated');

    const result = await generateLlmText({
      config: {} as any,
      input: 'hello',
      source: 'generic',
    });

    expect(result).toBe('generated');
    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.generateText, {
      request: {
        config: {} as any,
        input: 'hello',
        source: 'generic',
      },
    });
  });

  it('transcript llm job wrapper forwards the unified job request', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      taskId: 'translate-task-id',
      taskType: 'translate',
      jobHistoryId: 'history-a',
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true, translation: '你好' }],
    });

    const result = await runTranscriptLlmJob({
      taskId: 'translate-task-id',
      taskType: 'translate',
      jobHistoryId: 'history-a',
      config: {} as any,
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      targetLanguage: 'zh',
    });

    expect(result.segments?.[0].translation).toBe('你好');
    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.runTranscriptJob, {
      request: {
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: 'history-a',
        config: {} as any,
        segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
        targetLanguage: 'zh',
      },
    });
  });

  it('recognizer wrappers use the centralized recognizer commands', async () => {
    await initRecognizer({
      instanceId: 'record',
      asrRequest: {
        engine: 'local-sherpa',
        mode: 'streaming',
        modelPath: 'C:/models/live',
        numThreads: 4,
        enableItn: true,
        language: 'auto',
        punctuationModel: null,
        vadModel: null,
        vadBuffer: 5,
        modelType: 'sensevoice',
        hotwords: null,
        normalizationOptions: {
          enableTimeline: false,
        },
        postprocessOptions: {
          textReplacementSets: [],
          dropFinalDotSegments: true,
        },
      },
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.recognizer.init, {
      instanceId: 'record',
      asrRequest: {
        engine: 'local-sherpa',
        mode: 'streaming',
        modelPath: 'C:/models/live',
        numThreads: 4,
        enableItn: true,
        language: 'auto',
        punctuationModel: null,
        vadModel: null,
        vadBuffer: 5,
        modelType: 'sensevoice',
        hotwords: null,
        normalizationOptions: {
          enableTimeline: false,
        },
        postprocessOptions: {
          textReplacementSets: [],
          dropFinalDotSegments: true,
        },
      },
    });
  });

  it('automation wrappers centralize runtime rule calls', async () => {
    await replaceAutomationRuntimeRules([{ ruleId: 'rule-1' }]);

    expect(invoke).toHaveBeenCalledWith(TauriCommand.automation.replaceRuntimeRules, {
      rules: [{ ruleId: 'rule-1' }],
    });
  });

  it('project repository wrappers forward repository commands', async () => {
    const defaults = {
      summaryTemplateId: 'general',
      translationLanguage: 'zh',
      polishPresetId: 'general',
      exportFileNamePrefix: '',
      enabledTextReplacementSetIds: [],
      enabledHotwordSetIds: [],
      enabledPolishKeywordSetIds: [],
      enabledSpeakerProfileIds: [],
    };

    await projectList({
      fallbackEnabledPolishKeywordSetIds: ['keywords'],
      fallbackEnabledSpeakerProfileIds: ['speaker'],
    });
    await projectSaveAll([]);
    await projectCreate({ name: 'Research', description: 'Notes', icon: 'folder', defaults });
    await projectUpdate('project-1', { name: 'Updated' });
    await projectDelete('project-1');
    await projectReorder(['project-2', 'project-1']);
    await projectGetActiveId();
    await projectSetActiveId('project-2');

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.project.list, {
      fallbackEnabledPolishKeywordSetIds: ['keywords'],
      fallbackEnabledSpeakerProfileIds: ['speaker'],
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.project.saveAll, {
      projects: [],
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.project.create, {
      name: 'Research',
      description: 'Notes',
      icon: 'folder',
      defaults,
    });
    expect(invoke).toHaveBeenNthCalledWith(4, TauriCommand.project.update, {
      projectId: 'project-1',
      updates: { name: 'Updated' },
    });
    expect(invoke).toHaveBeenNthCalledWith(5, TauriCommand.project.delete, {
      projectId: 'project-1',
    });
    expect(invoke).toHaveBeenNthCalledWith(6, TauriCommand.project.reorder, {
      projectIds: ['project-2', 'project-1'],
    });
    expect(invoke).toHaveBeenNthCalledWith(7, TauriCommand.project.getActiveId);
    expect(invoke).toHaveBeenNthCalledWith(8, TauriCommand.project.setActiveId, {
      projectId: 'project-2',
    });
  });

  it('automation repository wrappers forward repository commands', async () => {
    const rule = {
      id: 'rule-1',
      name: 'Inbox',
      projectId: 'project-1',
      presetId: 'custom',
      watchDirectory: 'C:/watch',
      recursive: true,
      enabled: true,
      stageConfig: {
        autoPolish: false,
        autoTranslate: false,
        exportEnabled: true,
      },
      exportConfig: {
        directory: 'C:/exports',
        format: 'srt',
        mode: 'original',
      },
      createdAt: 1,
      updatedAt: 2,
    };
    const processedEntry = {
      ruleId: 'rule-1',
      filePath: 'C:/watch/meeting.wav',
      sourceFingerprint: 'fingerprint',
      size: 10,
      mtimeMs: 20,
      status: 'complete',
      processedAt: 30,
    };

    await automationLoadRepositoryState();
    await automationPersistRules([rule as any]);
    await automationPersistProcessedEntries([processedEntry as any]);
    await automationPersistRepositoryState([rule as any], [processedEntry as any]);
    await automationValidateRuleActivation(rule as any, {} as any, null);

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.automationRepository.loadState);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.automationRepository.persistRules, {
      rules: [rule],
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.automationRepository.persistProcessedEntries, {
      processedEntries: [processedEntry],
    });
    expect(invoke).toHaveBeenNthCalledWith(4, TauriCommand.automationRepository.persistState, {
      rules: [rule],
      processedEntries: [processedEntry],
    });
    expect(invoke).toHaveBeenNthCalledWith(5, TauriCommand.automationRepository.validateActivation, {
      rule,
      globalConfig: {},
      project: null,
    });
  });

  it('recovery wrappers forward repository commands', async () => {
    const item = {
      id: 'recovery-1',
      filename: 'meeting.wav',
      filePath: 'C:/watch/meeting.wav',
      source: 'batch_import',
      resolution: 'pending',
      progress: 10,
      segments: [],
      projectId: null,
      lastKnownStage: 'queued',
      updatedAt: 100,
      hasSourceFile: true,
      canResume: true,
    };
    const queueItem = {
      id: 'queue-1',
      filename: 'meeting.wav',
      filePath: 'C:/watch/meeting.wav',
      status: 'pending',
      progress: 10,
      segments: [],
      projectId: null,
    };
    vi.mocked(invoke).mockResolvedValueOnce({ version: 1, updatedAt: 100, items: [item] });

    const snapshot = await recoveryLoadSnapshot();
    await recoverySaveSnapshot([item as any]);
    await recoveryPersistQueueSnapshot([queueItem as any]);

    expect(snapshot).toEqual({ version: 1, updatedAt: 100, items: [item] });
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.recovery.loadSnapshot);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.recovery.saveSnapshot, {
      items: [item],
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.recovery.persistQueueSnapshot, {
      queueItems: [queueItem],
    });
  });

  it('task ledger wrappers forward repository commands', async () => {
    const record = {
      id: 'task-1',
      kind: 'batchImport',
      status: 'running',
      title: 'meeting.wav',
      progress: 50,
      createdAt: 100,
      updatedAt: 101,
      retryable: true,
      cancelable: true,
      recoverable: false,
    };
    vi.mocked(invoke).mockResolvedValue({ version: 1, updatedAt: 101, tasks: [record] });

    const snapshot = await taskLedgerLoadSnapshot();
    await taskLedgerUpsertTask(record as any);
    await taskLedgerPatchTask('task-1', { status: 'cancelRequested' });
    await taskLedgerRemoveTask('task-1');
    await taskLedgerClearResolved();

    expect(snapshot).toEqual({ version: 1, updatedAt: 101, tasks: [record] });
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.taskLedger.loadSnapshot);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.taskLedger.upsertTask, {
      record,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.taskLedger.patchTask, {
      id: 'task-1',
      patch: { status: 'cancelRequested' },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, TauriCommand.taskLedger.removeTask, {
      id: 'task-1',
    });
    expect(invoke).toHaveBeenNthCalledWith(5, TauriCommand.taskLedger.clearResolved);
  });

  it('llm usage wrappers forward analytics repository commands', async () => {
    await llmUsageEnsureStorage();
    await llmUsageReadRaw();
    await llmUsageReplaceRaw('{"schemaVersion":1}');

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.llmUsage.ensureStorage);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.llmUsage.readRaw);
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.llmUsage.replaceRaw, {
      content: '{"schemaVersion":1}',
    });
  });

  it('backup wrappers centralize import commands', async () => {
    await applyPreparedHistoryImport('import-1');

    expect(invoke).toHaveBeenCalledWith(TauriCommand.backup.applyPreparedImport, {
      importId: 'import-1',
    });
  });

  it('speaker wrappers centralize speaker processing commands', async () => {
    await annotateSpeakerSegmentsFromFile('C:/audio.wav', [], {} as any);
    await buildSpeakerReviewSnapshot([], 'pending');
    await applySpeakerProfileToGroup({
      segments: [],
      groupId: 'anonymous-1',
      targetProfileId: 'speaker-1',
      speakerProfiles: [],
      enabledSpeakerProfileIds: [],
    });
    await resetSpeakerGroupToAnonymous({
      segments: [],
      groupId: 'anonymous-1',
    });
    await confirmSpeakerGroupReview({
      segments: [],
      groupId: 'anonymous-1',
    });

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.speaker.annotateSegmentsFromFile, {
      filePath: 'C:/audio.wav',
      segments: [],
      speakerProcessing: {} as any,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.speaker.buildReviewSnapshot, {
      segments: [],
      activeFilter: 'pending',
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.speaker.applyProfileToGroup, {
      request: {
        segments: [],
        groupId: 'anonymous-1',
        targetProfileId: 'speaker-1',
        speakerProfiles: [],
        enabledSpeakerProfileIds: [],
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, TauriCommand.speaker.resetGroupToAnonymous, {
      request: {
        segments: [],
        groupId: 'anonymous-1',
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(5, TauriCommand.speaker.confirmGroupReview, {
      request: {
        segments: [],
        groupId: 'anonymous-1',
      },
    });
  });

  it('system wrappers centralize native text and cursor helpers', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([640, 360]);

    await injectText('hello', ['alt']);
    const mousePosition = await getMousePosition();

    expect(mousePosition).toEqual([640, 360]);
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.system.injectText, {
      text: 'hello',
      shortcutModifiers: ['alt'],
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.system.getMousePosition);
  });

  it('system aux-window wrappers preserve generic call sites', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce({ theme: 'dark' });

    await setAuxWindowState('voice-typing', { theme: 'dark' });
    const state = await getAuxWindowState<{ theme: string }>('voice-typing');

    expect(state).toEqual({ theme: 'dark' });
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.system.setAuxWindowState, {
      label: 'voice-typing',
      payload: { theme: 'dark' },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.system.getAuxWindowState, {
      label: 'voice-typing',
    });
  });

  it('exposes stable fixed events and the recognizer event builder', () => {
    expect(TauriEvent.tray.checkUpdates).toBe('check-updates');
    expect(TauriEvent.audio.microphonePeak).toBe('microphone-audio');
    expect(TauriEvent.llm.usageRecorded).toBe('llm-usage-recorded');
    expect(buildRecognizerOutputEvent('voice-typing')).toBe('recognizer-output-voice-typing');
  });
});
