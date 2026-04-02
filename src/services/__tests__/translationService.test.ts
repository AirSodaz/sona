import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { translationService } from '../translationService';
import { historyService } from '../historyService';
import { useTranscriptStore } from '../../stores/transcriptStore';

const mockListenToLlmTaskChunks = vi.fn();
const mockListenToLlmTaskProgress = vi.fn();
const mockCreateLlmTaskId = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../llmTaskService', () => ({
  createLlmTaskId: (...args: unknown[]) => mockCreateLlmTaskId(...args),
  listenToLlmTaskChunks: (...args: unknown[]) => mockListenToLlmTaskChunks(...args),
  listenToLlmTaskProgress: (...args: unknown[]) => mockListenToLlmTaskProgress(...args),
}));

vi.mock('../historyService', () => ({
  historyService: {
    loadTranscript: vi.fn(),
    updateTranscript: vi.fn(),
  },
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: {
    getState: vi.fn(),
    setState: vi.fn(),
    subscribe: vi.fn(),
  },
}));

describe('TranslationService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateLlmTaskId.mockReturnValue('translate-task-id');
    mockListenToLlmTaskChunks.mockResolvedValue(vi.fn());
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());
  });

  it('translates the active transcript incrementally and toggles visibility when finished', async () => {
    const mockStore = {
      config: {
        llmSettings: {
          activeProvider: 'open_ai',
          providers: {
            open_ai: {
              apiHost: 'test-url',
              apiKey: 'test-key',
              temperature: 0.7,
            },
          },
          models: {
            'open-ai-test': {
              id: 'open-ai-test',
              provider: 'open_ai',
              model: 'test-model',
            },
          },
          modelOrder: ['open-ai-test'],
          selections: {
            translationModelId: 'open-ai-test',
          },
        },
        translationLanguage: 'ja',
      },
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      updateSegment: vi.fn(),
      updateLlmState: vi.fn(),
      getLlmState: vi.fn().mockReturnValue({ isTranslationVisible: false }),
      sourceHistoryId: null,
    };

    (useTranscriptStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue(mockStore);
    mockListenToLlmTaskProgress.mockImplementation(async (_taskId, _taskType, onProgress) => {
      onProgress({ taskId: 'translate-task-id', taskType: 'translate', completedChunks: 1, totalChunks: 2 });
      return vi.fn();
    });
    mockListenToLlmTaskChunks.mockImplementation(async (_taskId, _taskType, onChunk) => {
      await onChunk({
        taskId: 'translate-task-id',
        taskType: 'translate',
        chunkIndex: 1,
        totalChunks: 2,
        items: [{ id: '1', translation: 'こんにちは' }],
      });
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: '1', translation: 'こんにちは' },
    ]);

    await translationService.translateCurrentTranscript();

    expect(invoke).toHaveBeenCalledWith('translate_transcript_segments', {
      request: {
        taskId: 'translate-task-id',
        config: expect.objectContaining({ apiKey: 'test-key' }),
        segments: [{ id: '1', text: 'hello' }],
        targetLanguage: 'ja',
      },
    });
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ isTranslating: true, translationProgress: 0 }, 'current');
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ translationProgress: 50 }, 'current');
    expect(mockStore.updateSegment).toHaveBeenCalledWith('1', { translation: 'こんにちは' });
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ translationProgress: 100 }, 'current');
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ isTranslating: false }, 'current');
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ isTranslationVisible: true }, 'current');
  });

  it('falls back to final result when no chunk event arrives', async () => {
    const mockStore = {
      config: {
        llmSettings: {
          activeProvider: 'open_ai',
          providers: {
            open_ai: {
              apiHost: 'test-url',
              apiKey: 'test-key',
              temperature: 0.7,
            },
          },
          models: {
            'open-ai-test': {
              id: 'open-ai-test',
              provider: 'open_ai',
              model: 'test-model',
            },
          },
          modelOrder: ['open-ai-test'],
          selections: {
            translationModelId: 'open-ai-test',
          },
        },
        translationLanguage: 'ja',
      },
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      updateSegment: vi.fn(),
      updateLlmState: vi.fn(),
      getLlmState: vi.fn().mockReturnValue({ isTranslationVisible: true }),
      sourceHistoryId: null,
    };

    (useTranscriptStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue(mockStore);
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', translation: 'こんにちは' }]);

    await translationService.translateCurrentTranscript();

    expect(mockStore.updateSegment).toHaveBeenCalledWith('1', { translation: 'こんにちは' });
  });

  it('updates background history when the active record changes mid-translation', async () => {
    const activeStore = {
      config: {
        llmSettings: {
          activeProvider: 'open_ai',
          providers: {
            open_ai: {
              apiHost: 'test-url',
              apiKey: 'test-key',
              temperature: 0.7,
            },
          },
          models: {
            'open-ai-test': {
              id: 'open-ai-test',
              provider: 'open_ai',
              model: 'test-model',
            },
          },
          modelOrder: ['open-ai-test'],
          selections: {
            translationModelId: 'open-ai-test',
          },
        },
        translationLanguage: 'zh',
      },
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      updateSegment: vi.fn(),
      updateLlmState: vi.fn(),
      getLlmState: vi.fn().mockReturnValue({ isTranslationVisible: true }),
      sourceHistoryId: 'history-a',
    };

    const switchedStore = {
      ...activeStore,
      sourceHistoryId: 'history-b',
    };

    (useTranscriptStore.getState as unknown as ReturnType<typeof vi.fn>)
      .mockReturnValueOnce(activeStore)
      .mockReturnValueOnce(switchedStore)
      .mockReturnValue(switchedStore);

    mockListenToLlmTaskChunks.mockImplementation(async (_taskId, _taskType, onChunk) => {
      await onChunk({
        taskId: 'translate-task-id',
        taskType: 'translate',
        chunkIndex: 1,
        totalChunks: 1,
        items: [{ id: '1', translation: '你好' }],
      });
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: '1', translation: '你好' },
    ]);
    (historyService.loadTranscript as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ]);

    await translationService.translateCurrentTranscript();

    expect(historyService.loadTranscript).toHaveBeenCalledWith('history-a.json');
    expect(historyService.updateTranscript).toHaveBeenCalledWith('history-a', [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true, translation: '你好' },
    ]);
  });
});
