import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { TauriCommand } from '../commands';
import { TauriEvent, buildRecognizerOutputEvent } from '../events';
import { invokeTauri } from '../invoke';
import {
  getAsrRuntimeMetrics,
  getDiagnosticsCoreSnapshot,
  getModelCatalogSnapshot,
  openLogFolder,
  setLogLevel,
  setMinimizeToTray,
} from '../app';
import { startMicrophoneCapture, stopSystemAudioCapture } from '../audio';
import {
  historyCreateTranscriptSnapshot,
  historyListTranscriptSnapshots,
  historyLoadTranscriptSnapshot,
  historyQueryWorkspace,
  historyUpdateTranscript,
} from '../history';
import { generateLlmText } from '../llm';
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
  annotateSpeakerSegmentsFromFile,
  applySpeakerProfileToGroup,
  buildSpeakerReviewSnapshot,
  confirmSpeakerGroupReview,
  resetSpeakerGroupToAnonymous,
} from '../speaker';
import { getAuxWindowState, getMousePosition, injectText, setAuxWindowState } from '../system';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('tauri boundary wrappers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

  it('app wrappers expose the diagnostics core snapshot', async () => {
    const coreSnapshot = {
      scannedAt: '2026-05-03T00:00:00.000Z',
      overview: [],
      sections: [],
      runtimeEnvironment: {
        ffmpegPath: 'C:/app/ffmpeg.exe',
        ffmpegExists: true,
        logDirPath: 'C:/app/logs',
      },
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
    expect(invoke).toHaveBeenCalledWith(TauriCommand.app.getDiagnosticsCoreSnapshot, {
      input,
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

  it('recognizer wrappers use the centralized recognizer commands', async () => {
    await initRecognizer({
      instanceId: 'record',
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
    });

    expect(invoke).toHaveBeenCalledWith(TauriCommand.recognizer.init, {
      instanceId: 'record',
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
