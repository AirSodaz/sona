import { beforeEach, describe, expect, it, vi } from 'vitest';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { invoke } from '@tauri-apps/api/core';
import type {
  RecoveredQueueItem_Serialize,
  RecoveryItemInput_Serialize,
  RustTauriCommandContractMap,
} from '../../../bindings';
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
  historyCleanupAudio,
  historyBuildTranscriptDiff,
  historyCreateLiveDraft,
  historyCreateTranscriptSnapshot,
  historyListTranscriptSnapshots,
  historyLoadTranscript,
  historyLoadTranscriptSnapshot,
  historyPreviewAudioCleanup,
  historyQueryWorkspace,
  historyRestoreTranscriptDiffRows,
  historySaveImportedFile,
  historySaveRecording,
  historySaveSummary,
  historyUpdateTranscript,
} from '../history';
import {
  completeLlm,
  describeLlmModel,
  generateLlmText,
  listLlmModels,
  polishTranscriptSegments,
  runTranscriptLlmJob,
  summarizeTranscript,
  translateTranscriptSegments,
} from '../llm';
import { initRecognizer, processBatchFile } from '../recognizer';
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
  storageClearWebviewBrowsingData,
  storageGetUsageSnapshot,
} from '../storage';
import {
  annotateSpeakerSegmentsFromFile,
  applySpeakerProfileToGroup,
  buildSpeakerReviewSnapshot,
  confirmSpeakerGroupReview,
  resetSpeakerGroupToAnonymous,
} from '../speaker';
import { getAuxWindowState, getMousePosition, injectText, setAuxWindowState } from '../system';
import {
  createSyncVault,
  joinSyncVault,
  previewSyncJoin,
  testWebDavSyncProvider,
} from '../sync';
import {
  taskLedgerClearResolved,
  taskLedgerLoadSnapshot,
  taskLedgerPatchTask,
  taskLedgerRemoveTask,
  taskLedgerUpsertTask,
} from '../taskLedger';
import { tagList, tagSaveAll } from '../tag';
import type { TaskLedgerRecord } from '../../../types/taskLedger';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const rustOwnedHistoryUpdateArgs: RustTauriCommandContractMap['history_update_transcript']['args'] = {
  historyId: 'history-typed-contract',
  segments: [],
};
void rustOwnedHistoryUpdateArgs;

const uiLlmConfig = {
  provider: 'open_ai',
  strategy: 'openai_compatible',
  baseUrl: 'https://api.openai.com',
  apiKey: 'test-key',
  model: 'gpt-4.1',
  temperature: 0.7,
  timeoutSeconds: 30,
} as const;

const coreLlmConfig = {
  provider: { Builtin: 'open_ai' },
  strategy: 'open_ai_compatible',
  baseUrl: 'https://api.openai.com',
  apiKey: 'test-key',
  model: 'gpt-4.1',
  apiPath: null,
  apiVersion: null,
  temperature: 0.7,
  reasoningEnabled: null,
  reasoningLevel: null,
  timeoutSeconds: 30,
} as const;

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
      models: [
        {
          id: 'live-model',
          name: 'Live Model',
          description: 'Streaming ASR model',
          url: 'https://example.com/live-model.tar.bz2',
          type: 'zipformer',
          modes: null,
          language: 'zh',
          size: '100 MB',
          isArchive: true,
          engine: 'sherpa-onnx',
          rules: {
            requiresVad: true,
            requiresPunctuation: false,
            timestampSupportHint: null,
          },
          installPath: 'C:/models/live-model',
          downloadPath: 'C:/models/live-model.tar.bz2',
          isInstalled: false,
        },
      ],
      sections: [],
      selectionOptions: {
        streaming: [],
        batch: [],
        speakerSegmentation: [],
        speakerEmbedding: [],
      },
      modelPathById: {},
      modelIdByNormalizedPath: {},
      pathMatchTokens: [],
      dependencyRequestsByModelId: {},
      restoreDefaults: {
        streamingModelPath: null,
        batchModelPath: null,
        vadModelPath: null,
        punctuationModelPath: null,
        speakerSegmentationModelPath: null,
        speakerEmbeddingModelPath: null,
        enableItn: true,
        batchVadEnabled: false,
        vadBufferSize: 0.5,
        maxConcurrent: 2,
      },
    };
    vi.mocked(invoke).mockResolvedValueOnce(snapshot);

    const result = await getModelCatalogSnapshot();

    expect(result.models[0]).not.toHaveProperty('modes');
    expect(result.models[0].rules).not.toHaveProperty('timestampSupportHint');
    expect(result.restoreDefaults).toEqual({
      enableITN: true,
      batchVadEnabled: false,
      vadBufferSize: 0.5,
      maxConcurrent: 2,
    });
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.getModelCatalogSnapshot);
  });

  it('rejects model catalog values outside the UI contract', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      modelsDir: 'C:/models',
      models: [{
        type: 'future-asr-engine',
        modes: null,
        engine: 'sherpa-onnx',
      }],
      sections: [],
    });

    await expect(getModelCatalogSnapshot()).rejects.toThrow(
      'Unexpected model catalog type: future-asr-engine',
    );
  });

  it('rejects an invalid model catalog VAD buffer size', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      modelsDir: 'C:/models',
      models: [],
      sections: [],
      selectionOptions: {
        streaming: [],
        batch: [],
        speakerSegmentation: [],
        speakerEmbedding: [],
      },
      modelPathById: {},
      modelIdByNormalizedPath: {},
      pathMatchTokens: [],
      dependencyRequestsByModelId: {},
      restoreDefaults: {
        enableItn: true,
        batchVadEnabled: false,
        vadBufferSize: null,
        maxConcurrent: 2,
      },
    });

    await expect(getModelCatalogSnapshot()).rejects.toThrow(
      'Expected a finite number for model catalog vadBufferSize',
    );
  });

  it('app wrappers resolve model catalog selected ids', async () => {
    const paths = {
      streamingModelPath: 'C:/models/live',
      batchModelPath: 'C:/models/batch',
      speakerSegmentationModelPath: '',
      speakerEmbeddingModelPath: 'C:/models/speaker.onnx',
    };
    const selectedIds = {
      streaming: 'live-model',
      batch: 'batch-model',
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
        batchModelPath: 'C:/models/batch',
        vadModelPath: '',
        punctuationModelPath: '',
        microphoneId: 'default',
      },
      selectedModels: {
        live: { id: 'live', name: 'Live Model' },
        batch: { id: 'batch', name: 'Batch Model' },
      },
      modelRules: {
        live: { requiresVad: false, requiresPunctuation: false },
        batch: { requiresVad: false, requiresPunctuation: false },
      },
      pathStatuses: {
        liveModel: { path: 'C:/models/live', kind: 'directory', error: null },
        batchModel: { path: 'C:/models/batch', kind: 'directory', error: null },
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
        batchModelPath: 'C:/models/batch',
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
      input: {
        config: input.config,
        permissionState: 'prompt',
        microphoneProbe: {
          options: [],
          available: false,
          errorMessage: null,
        },
        systemAudioProbe: {
          options: [],
          available: false,
          errorMessage: null,
        },
        voiceTypingReadiness: {
          state: 'off',
          lastErrorMessage: null,
        },
      },
    });

    vi.mocked(invoke).mockResolvedValueOnce({
      ...coreSnapshot,
      permissionState: 'unexpected',
    });
    await expect(getDiagnosticsCoreSnapshot(input)).rejects.toThrow(
      'Unexpected diagnostics permission state: unexpected',
    );
  });

  it('app wrappers delegate config migration and effective config resolution to Rust', async () => {
    const globalConfig = {
      configVersion: 6,
      streamingModelPath: '',
      batchModelPath: '',
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
      icon: '',
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
    const segment = {
      id: 'segment-1',
      text: 'hello',
      start: 0,
      end: 1,
      isFinal: true,
    };

    const result = await historyUpdateTranscript('history-1', [segment]);

    expect(result).toEqual({ id: 'history-1' });
    expect(invoke).toHaveBeenCalledWith(TauriCommand.history.updateTranscript, {
      historyId: 'history-1',
      segments: [{
        ...segment,
        timing: null,
        tokens: null,
        timestamps: null,
        durations: null,
        translation: null,
        speaker: null,
        speakerAttribution: null,
      }],
    });
  });

  it('history wrappers normalize nested transcript and summary inputs for Rust', async () => {
    const segment = {
      id: 'segment-1',
      text: 'hello',
      start: 0,
      end: 1,
      isFinal: true,
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce({ rows: [], changedCount: 0 })
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(undefined);

    await historyBuildTranscriptDiff([segment], []);
    await historyRestoreTranscriptDiffRows([{
      id: 'row-1',
      status: 'removed',
      snapshotSegment: segment,
      snapshotIndex: 0,
      currentIndex: null,
    }], ['row-1']);
    await historySaveSummary('history-1', { activeTemplateId: 'general' });

    const wireSegment = {
      ...segment,
      timing: null,
      tokens: null,
      timestamps: null,
      durations: null,
      translation: null,
      speaker: null,
      speakerAttribution: null,
    };
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.history.buildTranscriptDiff, {
      snapshotSegments: [wireSegment],
      currentSegments: [],
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.history.restoreTranscriptDiffRows, {
      rows: [{
        id: 'row-1',
        status: 'removed',
        snapshotSegment: wireSegment,
        currentSegment: null,
        snapshotIndex: 0,
        currentIndex: null,
      }],
      selectedRowIds: ['row-1'],
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.history.saveSummary, {
      historyId: 'history-1',
      summaryPayload: { activeTemplateId: 'general', record: null },
    });
  });

  it('history wrappers forward item creation intents without caller-built items', async () => {
    await historyCreateLiveDraft(null, 'webm', 'project-1', 'system:mic');
    await historySaveRecording({
      segments: [],
      duration: 3,
      tagIds: ['project-1'],
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
      tagIds: ['project-1'],
      icon: 'system:mic',
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.history.saveRecording, {
      segments: [],
      duration: 3,
      tagIds: ['project-1'],
      audioBytes: [1, 2, 3],
      audioExtension: 'webm',
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.history.saveImportedFile, {
      sourcePath: 'D:/audio/meeting.mp3',
      segments: [],
      duration: 4,
      tagIds: [],
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

  it('history transcript reads normalize nullable wire fields for the editor model', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{
      id: 'segment-1',
      text: 'hello',
      start: 0,
      end: 1,
      isFinal: true,
      timing: null,
      tokens: null,
      timestamps: null,
      durations: null,
      translation: null,
      speaker: { id: 'speaker-1', label: 'Speaker 1', kind: 'anonymous', score: null },
      speakerAttribution: null,
    }]);

    const result = await historyLoadTranscript('history-1');

    expect(result).toEqual([{
      id: 'segment-1',
      text: 'hello',
      start: 0,
      end: 1,
      isFinal: true,
      timing: undefined,
      tokens: undefined,
      timestamps: undefined,
      durations: undefined,
      translation: undefined,
      speaker: { id: 'speaker-1', label: 'Speaker 1', kind: 'anonymous', score: undefined },
      speakerAttribution: undefined,
    }]);
  });

  it('history workspace query wrapper forwards flat query args', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      filteredItems: [],
      searchMatchByItemId: {},
      filteredItemCount: 0,
      hasMore: false,
      summary: {
        totalItems: 0,
        totalDuration: 0,
        latestTimestamp: null,
        recordingCount: 0,
        batchCount: 0,
      },
      itemCounts: { inbox: 0, byProjectId: {} },
    });

    await historyQueryWorkspace({
      scope: { kind: 'project', projectId: 'project-1' },
      query: 'roadmap',
      filterType: 'recording',
      dateFilter: 'week',
      sortOrder: 'title_asc',
      limit: 100,
      offset: 0,
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.history.queryWorkspace, {
      scope: { kind: 'project', projectId: 'project-1' },
      query: 'roadmap',
      filterType: 'recording',
      dateFilter: 'week',
      sortOrder: 'title_asc',
      limit: 100,
      offset: 0,
    });
  });

  it('history audio cleanup wrappers forward retention and active-exclusion args', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({
        eligibleCount: 2,
        removedCount: 2,
        removedBytes: 123,
        missingMarkedCount: 0,
        failedCount: 0,
        skippedActiveCount: 1,
      })
      .mockResolvedValueOnce({
        eligibleCount: 1,
        removedCount: 1,
        removedBytes: 64,
        missingMarkedCount: 0,
        failedCount: 0,
        skippedActiveCount: 0,
      });

    await historyPreviewAudioCleanup({
      retentionDays: 30,
      excludeHistoryId: 'history-open',
    });
    await historyCleanupAudio({
      retentionDays: null,
      excludeHistoryId: null,
    });

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.history.previewAudioCleanup, {
      retentionDays: 30,
      excludeHistoryId: 'history-open',
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.history.cleanupAudio, {
      retentionDays: null,
      excludeHistoryId: null,
    });
  });

  it('storage wrappers expose usage snapshot and WebView browsing-data cleanup', async () => {
    const snapshot = {
      generatedAt: '2026-07-04T00:00:00Z',
      totalBytes: 123,
      categories: {
        audio: {
          bytes: 10,
          historyAudioBytes: 6,
          speakerSampleBytes: 4,
          fileCount: 2,
        },
        database: {
          bytes: 20,
          sqlite: {
            mainDbBytes: 12,
            mainWalBytes: 0,
            mainShmBytes: 0,
            analyticsDbBytes: 8,
            analyticsWalBytes: 0,
            analyticsShmBytes: 0,
            dataBytes: 14,
            indexBytes: 6,
            freePageBytes: 0,
            indexEntries: [],
            dbstatAvailable: true,
          },
        },
        models: { bytes: 30, fileCount: 1 },
        temporary: { bytes: 40, fileCount: 1 },
        webviewCache: { bytes: 5, clearSupported: true, path: 'C:/App/EBWebView' },
        other: { bytes: 18, fileCount: 1 },
      },
    };
    const clearResult = {
      beforeBytes: 5,
      afterBytes: 1,
      clearRequested: true,
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce(clearResult);

    await expect(storageGetUsageSnapshot()).resolves.toEqual(snapshot);
    await expect(storageClearWebviewBrowsingData()).resolves.toEqual(clearResult);

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.storage.getUsageSnapshot);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.storage.clearWebviewBrowsingData);
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
      config: uiLlmConfig,
      input: 'hello',
      source: 'generic',
    });

    expect(result).toBe('generated');
    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.generateText, {
      request: {
        config: coreLlmConfig,
        input: 'hello',
        source: 'generic',
      },
    });
  });

  it('typed llm wrappers preserve completion and model-description IPC shapes', async () => {
    const response = {
      text: '{"answer":true}',
      json: { answer: true },
      usage: null,
      execution: {
        requestedFormat: 'json_object' as const,
        appliedFormat: 'json_object' as const,
        warnings: [],
        attempts: 1,
      },
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce(response)
      .mockResolvedValueOnce({ model: 'gpt-4.1', displayName: 'GPT-4.1' });

    const request = {
      config: uiLlmConfig,
      input: 'answer',
      options: {
        maxOutputTokens: 4096,
        responseFormat: { type: 'json_object' as const },
      },
    };
    await expect(completeLlm(request)).resolves.toEqual(response);
    await expect(describeLlmModel(uiLlmConfig)).resolves.toEqual(
      expect.objectContaining({ displayName: 'GPT-4.1' }),
    );

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.llm.complete, {
      request: {
        config: coreLlmConfig,
        systemPrompt: null,
        input: 'answer',
        options: {
          temperature: null,
          maxOutputTokens: 4096,
          reasoningEnabled: null,
          reasoningLevel: null,
          responseFormat: { type: 'json_object' },
          promptCache: 'disabled',
          capabilityPolicy: 'compatible',
        },
        source: null,
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.llm.describeModel, {
      config: coreLlmConfig,
    });
  });

  it('normalizes model-list providers and legacy strategy spellings', async () => {
    const models = [{ model: 'gpt-4.1', inputPrice: 0.01, contextWindow: 1_000_000 }];
    vi.mocked(invoke).mockResolvedValueOnce(models);

    await expect(listLlmModels({
      provider: 'open_ai',
      strategy: 'openai_compatible',
      baseUrl: 'https://api.openai.com',
      apiKey: 'test-key',
    })).resolves.toEqual(models);

    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.listModels, {
      request: {
        provider: { Builtin: 'open_ai' },
        strategy: 'open_ai_compatible',
        baseUrl: 'https://api.openai.com',
        apiKey: 'test-key',
      },
    });
  });

  it('rejects non-finite and unsafe LLM request numbers before invoking Tauri', async () => {
    await expect(completeLlm({
      config: { ...uiLlmConfig, temperature: Number.POSITIVE_INFINITY },
      input: 'answer',
    })).rejects.toThrow('request.config.temperature must be a finite number');

    await expect(completeLlm({
      config: uiLlmConfig,
      input: 'answer',
      options: { maxOutputTokens: Number.MAX_SAFE_INTEGER + 1 },
    })).rejects.toThrow('request.options.maxOutputTokens must be a non-negative safe integer');

    await expect(completeLlm({
      config: uiLlmConfig,
      input: 'answer',
      options: {
        responseFormat: {
          type: 'json_schema',
          name: 'answer',
          schema: { maximum: Number.POSITIVE_INFINITY },
        },
      },
    })).rejects.toThrow('request.options.responseFormat.schema.maximum must be a finite number');

    expect(invoke).not.toHaveBeenCalled();
  });

  it('rejects non-finite and unsafe LLM response metadata at the Tauri boundary', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([
      { model: 'broken-price', inputPrice: Number.POSITIVE_INFINITY },
    ]);

    await expect(listLlmModels({
      provider: 'open_ai',
      baseUrl: 'https://api.openai.com',
      apiKey: 'test-key',
    })).rejects.toThrow('result[0].inputPrice must be a finite number');

    vi.mocked(invoke).mockResolvedValueOnce([
      { model: 'unsafe-window', contextWindow: Number.MAX_SAFE_INTEGER + 1 },
    ]);

    await expect(listLlmModels({
      provider: 'open_ai',
      baseUrl: 'https://api.openai.com',
      apiKey: 'test-key',
    })).rejects.toThrow('result[0].contextWindow must be a non-negative safe integer');
  });

  it('rejects unsafe dynamic JSON numbers in LLM completion responses', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      text: '{}',
      json: { nested: [Number.MAX_SAFE_INTEGER + 1] },
      usage: null,
      execution: {
        requestedFormat: 'json_object',
        appliedFormat: 'json_object',
        warnings: [],
        attempts: 1,
      },
    });

    await expect(completeLlm({
      config: uiLlmConfig,
      input: 'answer',
    })).rejects.toThrow('result.json.nested[0] must be a safe integer');
  });

  it('normalizes all transcript task requests to generated Core contracts', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({
        templateId: 'default',
        content: 'summary',
      });

    await polishTranscriptSegments({
      taskId: 'polish-task',
      config: uiLlmConfig,
      segments: [{ id: '1', text: 'hello' }],
    });
    await translateTranscriptSegments({
      taskId: 'translate-task',
      config: uiLlmConfig,
      segments: [{ id: '1', text: 'hello' }],
      targetLanguage: 'zh',
    });
    await summarizeTranscript({
      taskId: 'summary-task',
      config: uiLlmConfig,
      template: {
        id: 'default',
        name: 'Default',
        instructions: 'Summarize.',
        builtIn: true,
      },
      segments: [{ id: '1', text: 'hello', start: 0, end: 1, isFinal: true }],
    });

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.llm.polishTranscriptSegments, {
      request: {
        taskId: 'polish-task',
        config: coreLlmConfig,
        segments: [{ id: '1', text: 'hello' }],
        chunkSize: null,
        context: null,
        keywords: null,
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.llm.translateTranscriptSegments, {
      request: {
        taskId: 'translate-task',
        config: coreLlmConfig,
        segments: [{ id: '1', text: 'hello' }],
        chunkSize: null,
        targetLanguage: 'zh',
        targetLanguageName: null,
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.llm.summarizeTranscript, {
      request: {
        taskId: 'summary-task',
        config: coreLlmConfig,
        template: { id: 'default', name: 'Default', instructions: 'Summarize.' },
        segments: [{ id: '1', text: 'hello', start: 0, end: 1, isFinal: true }],
        chunkCharBudget: null,
      },
    });
  });

  it('rejects unsafe transcript task chunk sizes before invoking Tauri', async () => {
    await expect(polishTranscriptSegments({
      taskId: 'polish-task',
      config: uiLlmConfig,
      segments: [{ id: '1', text: 'hello' }],
      chunkSize: -1,
    })).rejects.toThrow('request.chunkSize must be a non-negative safe integer');

    expect(invoke).not.toHaveBeenCalled();
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
      config: uiLlmConfig,
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      targetLanguage: 'zh',
    });

    expect(result.segments?.[0].translation).toBe('你好');
    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.runTranscriptJob, {
      request: {
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: 'history-a',
        config: coreLlmConfig,
        segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
        targetLanguage: 'zh',
        targetLanguageName: null,
        context: null,
        keywords: null,
        template: null,
        chunkSize: null,
        chunkCharBudget: null,
      },
    });
  });

  it('derives the Core default strategy for custom LLM providers', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('ok');

    await generateLlmText({
      config: {
        provider: 'custom-claude-gateway',
        baseUrl: 'https://llm.example.com',
        apiKey: 'test-key',
        model: 'claude-compatible',
      },
      input: 'hello',
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.generateText, {
      request: {
        config: {
          provider: { Custom: 'custom-claude-gateway' },
          strategy: 'open_ai_compatible',
          baseUrl: 'https://llm.example.com',
          apiKey: 'test-key',
          model: 'claude-compatible',
          apiPath: null,
          apiVersion: null,
          temperature: null,
          reasoningEnabled: null,
          reasoningLevel: null,
          timeoutSeconds: null,
        },
        input: 'hello',
        source: null,
      },
    });
  });

  it('derives the Core default strategy for built-in LLM providers', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('ok');

    await generateLlmText({
      config: {
        provider: 'open_ai',
        baseUrl: 'https://api.openai.com',
        apiKey: 'test-key',
        model: 'gpt-4.1',
      },
      input: 'hello',
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.generateText, {
      request: {
        config: {
          provider: { Builtin: 'open_ai' },
          strategy: 'open_ai',
          baseUrl: 'https://api.openai.com',
          apiKey: 'test-key',
          model: 'gpt-4.1',
          apiPath: null,
          apiVersion: null,
          temperature: null,
          reasoningEnabled: null,
          reasoningLevel: null,
          timeoutSeconds: null,
        },
        input: 'hello',
        source: null,
      },
    });
  });

  it('drops stale fields outside the selected transcript LLM job variant', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      taskId: 'translate-task-id',
      taskType: 'translate',
      segments: [],
    });

    await runTranscriptLlmJob({
      taskId: 'translate-task-id',
      taskType: 'translate',
      config: uiLlmConfig,
      segments: [],
      targetLanguage: 'zh',
      context: 'stale polish context',
      keywords: 'stale polish keywords',
      template: {
        id: 'stale-summary-template',
        name: 'Stale',
        instructions: 'Do not forward.',
        builtIn: false,
      },
      chunkCharBudget: 2048,
    } as unknown as Parameters<typeof runTranscriptLlmJob>[0]);

    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.runTranscriptJob, {
      request: {
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: null,
        config: coreLlmConfig,
        segments: [],
        targetLanguage: 'zh',
        targetLanguageName: null,
        context: null,
        keywords: null,
        template: null,
        chunkSize: null,
        chunkCharBudget: null,
      },
    });
  });

  it('keeps only polish fields in the normalized transcript LLM job', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      taskId: 'polish-task-id',
      taskType: 'polish',
      segments: [],
    });

    await runTranscriptLlmJob({
      taskId: 'polish-task-id',
      taskType: 'polish',
      config: uiLlmConfig,
      segments: [],
      context: 'meeting transcript',
      keywords: 'Sona',
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.runTranscriptJob, {
      request: {
        taskId: 'polish-task-id',
        taskType: 'polish',
        jobHistoryId: null,
        config: coreLlmConfig,
        segments: [],
        targetLanguage: null,
        targetLanguageName: null,
        context: 'meeting transcript',
        keywords: 'Sona',
        template: null,
        chunkSize: null,
        chunkCharBudget: null,
      },
    });
  });

  it('keeps only the Core summary template in the normalized transcript LLM job', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      taskId: 'summary-task-id',
      taskType: 'summary',
      summary: null,
    });

    await runTranscriptLlmJob({
      taskId: 'summary-task-id',
      taskType: 'summary',
      config: uiLlmConfig,
      segments: [],
      template: {
        id: 'general',
        name: 'General',
        instructions: 'Summarize.',
        builtIn: true,
      },
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.llm.runTranscriptJob, {
      request: {
        taskId: 'summary-task-id',
        taskType: 'summary',
        jobHistoryId: null,
        config: coreLlmConfig,
        segments: [],
        targetLanguage: null,
        targetLanguageName: null,
        context: null,
        keywords: null,
        template: {
          id: 'general',
          name: 'General',
          instructions: 'Summarize.',
        },
        chunkSize: null,
        chunkCharBudget: null,
      },
    });
  });

  it('recognizer wrappers use the centralized recognizer commands', async () => {
    await initRecognizer({
      instanceId: 'record',
      asrRequest: {
        engine: 'local-sherpa',
        mode: 'streaming',
        modelId: null,
        modelPath: 'C:/models/live',
        numThreads: 4,
        enableItn: true,
        language: 'auto',
        punctuationModel: null,
        vadModel: null,
        vadBuffer: 5,
        modelType: 'sensevoice',
        fileConfig: {
          encoder: 'encoder.onnx',
        },
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
        modelId: null,
        modelPath: 'C:/models/live',
        numThreads: 4,
        enableItn: true,
        language: 'auto',
        punctuationModel: null,
        vadModel: null,
        vadBuffer: 5,
        modelType: 'sensevoice',
        fileConfig: {
          encoder: 'encoder.onnx',
          decoder: null,
          model: null,
          joiner: null,
          tokens: null,
          convFrontend: null,
          encoderAdaptor: null,
          llm: null,
          embedding: null,
          tokenizer: null,
        },
        hotwords: null,
        speakerProcessing: null,
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

  it('normalizes batch speaker processing for the generated Core contract', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);

    await processBatchFile({
      filePath: 'C:/audio.wav',
      saveToPath: null,
      speakerProcessing: {
        speakerSegmentationModelPath: 'C:/models/seg.onnx',
        speakerEmbeddingModelPath: 'C:/models/embed.onnx',
        speakerProfiles: [{
          id: 'profile-1',
          name: 'Alice',
          enabled: true,
          samples: [{
            id: 'sample-1',
            filePath: 'C:/samples/alice.wav',
            sourceName: 'alice.wav',
            durationSeconds: 1.5,
          }],
        }],
      },
      asrRequest: {
        engine: 'local-sherpa',
        mode: 'batch',
        modelId: null,
        modelPath: 'C:/models/batch',
        numThreads: 4,
        enableItn: false,
        language: 'auto',
        punctuationModel: null,
        vadModel: null,
        vadBuffer: 5,
        modelType: 'sensevoice',
        hotwords: null,
        normalizationOptions: { enableTimeline: true },
        postprocessOptions: {
          textReplacementSets: [],
          dropFinalDotSegments: true,
        },
      },
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.recognizer.processBatchFile, {
      filePath: 'C:/audio.wav',
      saveToPath: null,
      speakerProcessing: {
        speakerSegmentationModelPath: 'C:/models/seg.onnx',
        speakerEmbeddingModelPath: 'C:/models/embed.onnx',
        speakerProfiles: [{
          id: 'profile-1',
          name: 'Alice',
          enabled: true,
          samples: [{
            id: 'sample-1',
            filePath: 'C:/samples/alice.wav',
            sourceName: 'alice.wav',
            durationSeconds: 1.5,
          }],
        }],
      },
      asrRequest: expect.objectContaining({
        engine: 'local-sherpa',
        mode: 'batch',
        speakerProcessing: null,
      }),
    });
  });

  it('rejects non-finite ASR transport numbers before invoking Tauri', async () => {
    await expect(initRecognizer({
      instanceId: 'record',
      asrRequest: {
        engine: 'local-sherpa',
        mode: 'streaming',
        modelId: null,
        modelPath: 'C:/models/live',
        numThreads: 4,
        enableItn: true,
        language: 'auto',
        punctuationModel: null,
        vadModel: null,
        vadBuffer: Number.POSITIVE_INFINITY,
        modelType: 'sensevoice',
        hotwords: null,
        normalizationOptions: { enableTimeline: false },
        postprocessOptions: {
          textReplacementSets: [],
          dropFinalDotSegments: true,
        },
      },
    })).rejects.toThrow('asrRequest.vadBuffer must be a finite number');

    expect(invoke).not.toHaveBeenCalled();
  });

  it('automation wrappers centralize runtime rule calls', async () => {
    const rule = {
      ruleId: 'rule-1',
      watchDirectory: 'C:/watch',
      recursive: true,
      excludeDirectory: 'C:/exports',
      debounceMs: 250,
      stableWindowMs: 5000,
    };
    vi.mocked(invoke).mockResolvedValueOnce([]);

    await replaceAutomationRuntimeRules([rule]);

    expect(invoke).toHaveBeenCalledWith(TauriCommand.automation.replaceRuntimeRules, {
      rules: [rule],
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
    const wireProject = {
      id: 'project-1',
      name: 'Research',
      description: 'Notes',
      icon: 'folder',
      createdAt: 100,
      updatedAt: 101,
      defaults: {
        ...defaults,
        polishScenario: null,
        polishContext: null,
      },
    };
    const project = {
      ...wireProject,
      color: '#123456',
      sortOrder: 4,
      defaults,
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce([wireProject])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(wireProject)
      .mockResolvedValueOnce(wireProject)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([wireProject])
      .mockResolvedValueOnce('project-1')
      .mockResolvedValueOnce(undefined);

    const listed = await projectList({
      fallbackEnabledPolishKeywordSetIds: ['keywords'],
      fallbackEnabledSpeakerProfileIds: ['speaker'],
    });
    await projectSaveAll([project]);
    await projectCreate({ name: 'Research', description: 'Notes', icon: 'folder', defaults });
    await projectUpdate('project-1', { name: 'Updated' });
    await projectDelete('project-1');
    await projectReorder(['project-2', 'project-1']);
    await projectGetActiveId();
    await projectSetActiveId('project-2');

    expect(listed).toEqual([{
      ...wireProject,
      color: '#64748b',
      sortOrder: 0,
      defaults,
    }]);

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.project.list, {
      fallbackEnabledPolishKeywordSetIds: ['keywords'],
      fallbackEnabledSpeakerProfileIds: ['speaker'],
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.project.saveAll, {
      projects: [wireProject],
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

  it('tag repository wrappers normalize records at the Tauri boundary', async () => {
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
    const wireTag = {
      id: 'tag-1',
      name: 'Research',
      description: 'Notes',
      icon: 'folder',
      color: '#123456',
      sortOrder: 4,
      createdAt: 100,
      updatedAt: 101,
      defaults: {
        ...defaults,
        polishScenario: null,
        polishContext: null,
      },
    };
    const tag = { ...wireTag, defaults };
    vi.mocked(invoke)
      .mockResolvedValueOnce([wireTag])
      .mockResolvedValueOnce(undefined);

    const listed = await tagList();
    await tagSaveAll([tag]);

    expect(listed).toEqual([tag]);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.tag.saveAll, {
      tags: [wireTag],
    });
  });

  it('sync wrappers convert WebDAV UI inputs to provider-neutral lifecycle requests', async () => {
    const webdav = {
      serverUrl: 'https://dav.example.com',
      remoteRoot: 'sona',
      username: 'alice',
      password: 'secret',
    };
    const provider = {
      providerId: 'webdav',
      configuration: webdav,
    };
    vi.mocked(invoke).mockResolvedValue({});

    await testWebDavSyncProvider(webdav);
    await createSyncVault({
      provider: webdav,
      preset: 'standard',
      masterPassword: 'master-password',
      createRecoveryKey: true,
    });
    await previewSyncJoin({
      provider: webdav,
      vaultId: 'vault-1',
      masterPassword: 'master-password',
    });
    await joinSyncVault({
      provider: webdav,
      vaultId: 'vault-1',
      masterPassword: 'master-password',
    });

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.sync.testProvider, {
      provider,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.sync.createVault, {
      request: {
        provider,
        preset: 'standard',
        masterPassword: 'master-password',
        createRecoveryKey: true,
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.sync.previewJoin, {
      request: {
        provider,
        vaultId: 'vault-1',
        masterPassword: 'master-password',
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, TauriCommand.sync.joinVault, {
      request: {
        provider,
        vaultId: 'vault-1',
        masterPassword: 'master-password',
      },
    });
  });

  it('automation repository wrappers forward repository commands', async () => {
    const rule = {
      id: 'rule-1',
      name: 'Inbox',
      saveHistory: true,
      tagIds: ['project-1'],
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
      id: 'entry-1',
      ruleId: 'rule-1',
      filePath: 'C:/watch/meeting.wav',
      sourceFingerprint: 'fingerprint',
      size: 10,
      mtimeMs: 20,
      status: 'complete',
      processedAt: 30,
    };
    vi.mocked(invoke).mockResolvedValueOnce({
      rules: [{
        ...rule,
        stageConfig: {
          ...rule.stageConfig,
          polishPresetId: '',
          translationLanguage: '',
        },
        exportConfig: { ...rule.exportConfig, prefix: '' },
      }],
      processedEntries: [{
        ...processedEntry,
        historyId: null,
        exportPath: null,
        errorMessage: null,
      }],
    });

    const state = await automationLoadRepositoryState();
    await automationPersistRules([rule as any]);
    await automationPersistProcessedEntries([processedEntry as any]);
    await automationPersistRepositoryState([rule as any], [processedEntry as any]);
    await automationValidateRuleActivation(rule as any, {} as any, null);

    expect(state).toEqual({
      rules: [{
        ...rule,
        stageConfig: rule.stageConfig,
        exportConfig: rule.exportConfig,
      }],
      processedEntries: [processedEntry],
    });

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
      tags: [],
    });
  });

  it('recovery wrappers forward repository commands', async () => {
    const segment = {
      id: 'segment-1',
      text: 'hello',
      start: 0,
      end: 1,
      isFinal: true,
    };
    const item: RecoveredQueueItem_Serialize = {
      id: 'recovery-1',
      filename: 'meeting.wav',
      filePath: 'C:/watch/meeting.wav',
      source: 'batch_import',
      resolution: 'pending',
      progress: 10,
      segments: [segment],
      tagIds: [],
      lastKnownStage: 'queued',
      updatedAt: 100,
      hasSourceFile: true,
      canResume: true,
      exportConfig: null,
      stageConfig: null,
    };
    const queueItem: RecoveryItemInput_Serialize = {
      id: 'queue-1',
      filename: 'meeting.wav',
      filePath: 'C:/watch/meeting.wav',
      status: 'pending',
      progress: 10,
      segments: [segment],
      projectId: null,
    };
    vi.mocked(invoke).mockResolvedValueOnce({ version: 1, updatedAt: 100, items: [item] });

    const snapshot = await recoveryLoadSnapshot();
    await recoverySaveSnapshot([item]);
    await recoveryPersistQueueSnapshot([queueItem]);

    expect(snapshot).toEqual({ version: 1, updatedAt: 100, items: [item] });
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.recovery.loadSnapshot);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.recovery.saveSnapshot, {
      items: [{
        ...item,
        segments: [{
          ...segment,
          tokens: null,
          timestamps: null,
          durations: null,
          translation: null,
          speaker: null,
          speakerAttribution: null,
        }],
      }],
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.recovery.persistQueueSnapshot, {
      queueItems: [{
        ...queueItem,
        segments: [{
          ...segment,
          tokens: null,
          timestamps: null,
          durations: null,
          translation: null,
          speaker: null,
          speakerAttribution: null,
        }],
      }],
    });
  });

  it('task ledger wrappers forward repository commands', async () => {
    const record: TaskLedgerRecord = {
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
      projectId: 'tag-1',
    };
    const { projectId, ...recordWithoutProjectId } = record;
    const wireRecord = {
      ...recordWithoutProjectId,
      stage: null,
      historyId: null,
      tagIds: projectId ? [projectId] : [],
      filePath: null,
      automationRuleId: null,
      sourceFingerprint: null,
      errorMessage: null,
      templateId: null,
      targetLanguage: null,
    };
    vi.mocked(invoke).mockResolvedValue({ version: 1, updatedAt: 101, tasks: [wireRecord] });

    const snapshot = await taskLedgerLoadSnapshot();
    await taskLedgerUpsertTask(record);
    await taskLedgerPatchTask('task-1', {
      status: 'cancelRequested',
      projectId: 'tag-2',
    });
    await taskLedgerRemoveTask('task-1');
    await taskLedgerClearResolved();

    expect(snapshot).toEqual({
      version: 1,
      updatedAt: 101,
      tasks: [{
        ...record,
        projectId: undefined,
        tagIds: ['tag-1'],
      }],
    });
    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.taskLedger.loadSnapshot);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.taskLedger.upsertTask, {
      record: wireRecord,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, TauriCommand.taskLedger.patchTask, {
      id: 'task-1',
      patch: { status: 'cancelRequested', tagIds: ['tag-2'] },
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
