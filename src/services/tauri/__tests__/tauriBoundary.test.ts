import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { TauriCommand } from '../commands';
import { TauriEvent, buildRecognizerOutputEvent } from '../events';
import { invokeTauri } from '../invoke';
import { openLogFolder, setMinimizeToTray } from '../app';
import { startMicrophoneCapture, stopSystemAudioCapture } from '../audio';
import {
  historyCreateTranscriptSnapshot,
  historyListTranscriptSnapshots,
  historyLoadTranscriptSnapshot,
  historyUpdateTranscript,
} from '../history';
import { generateLlmText } from '../llm';
import { initRecognizer } from '../recognizer';
import { replaceAutomationRuntimeRules } from '../automation';
import { applyPreparedHistoryImport } from '../backup';
import { annotateSpeakerSegmentsFromFile } from '../speaker';
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

    expect(invoke).toHaveBeenNthCalledWith(1, TauriCommand.app.openLogFolder);
    expect(invoke).toHaveBeenNthCalledWith(2, TauriCommand.app.setMinimizeToTray, {
      enabled: false,
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
    await historyUpdateTranscript('history-1', [], 'preview', 'search');

    expect(invoke).toHaveBeenCalledWith(TauriCommand.history.updateTranscript, {
      historyId: 'history-1',
      segments: [],
      previewText: 'preview',
      searchContent: 'search',
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

  it('backup wrappers centralize import commands', async () => {
    await applyPreparedHistoryImport('import-1');

    expect(invoke).toHaveBeenCalledWith(TauriCommand.backup.applyPreparedImport, {
      importId: 'import-1',
    });
  });

  it('speaker wrappers centralize speaker processing commands', async () => {
    await annotateSpeakerSegmentsFromFile('C:/audio.wav', [], {} as any);

    expect(invoke).toHaveBeenCalledWith(TauriCommand.speaker.annotateSegmentsFromFile, {
      filePath: 'C:/audio.wav',
      segments: [],
      speakerProcessing: {} as any,
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
