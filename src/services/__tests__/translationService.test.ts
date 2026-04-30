import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { translationService } from '../translationService';
import { historyService } from '../historyService';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';
import type { AppConfig } from '../../types/config';
import { buildTestConfig as buildBaseTestConfig, type DeepPartial } from '../../test-utils/configTestUtils';

const mockListenToLlmTaskChunks = vi.fn();
const mockListenToLlmTaskProgress = vi.fn();
const mockCreateLlmTaskId = vi.fn();

const TEST_LLM_SETTINGS = {
  activeProvider: 'open_ai',
  providers: {
    open_ai: {
      apiHost: 'test-url',
      apiKey: 'test-key',
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
} satisfies NonNullable<AppConfig['llmSettings']>;

type TestConfigOverrides = DeepPartial<AppConfig>;

function buildTranslationTestConfig(overrides: TestConfigOverrides = {}): AppConfig {
  const baseLlmSettings = buildBaseTestConfig({
    llmSettings: TEST_LLM_SETTINGS,
  }).llmSettings!;
  const overrideLlmSettings = overrides.llmSettings;

  return buildBaseTestConfig({
    ...overrides,
    llmSettings: {
      ...baseLlmSettings,
      ...overrideLlmSettings,
      providers: {
        ...baseLlmSettings.providers,
        ...overrideLlmSettings?.providers,
      },
      models: {
        ...baseLlmSettings.models,
        ...overrideLlmSettings?.models,
      },
      modelOrder: overrideLlmSettings?.modelOrder ?? baseLlmSettings.modelOrder,
      selections: {
        ...baseLlmSettings.selections,
        ...overrideLlmSettings?.selections,
      },
    },
  });
}

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

describe('TranslationService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTranscriptStores();
    mockCreateLlmTaskId.mockReturnValue('translate-task-id');
    mockListenToLlmTaskChunks.mockResolvedValue(vi.fn());
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());
  });

  it('translates the active transcript incrementally and toggles visibility when finished', async () => {
    useTranscriptStore.setState({
      config: buildTranslationTestConfig({
        translationLanguage: 'ja',
      }),
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      sourceHistoryId: null,
    });
    mockListenToLlmTaskProgress.mockImplementation(async (_taskId, _taskType, onProgress) => {
      onProgress({ taskId: 'translate-task-id', taskType: 'translate', completedChunks: 1, totalChunks: 2 });
      expect(useTranscriptStore.getState().getLlmState('current').translationProgress).toBe(50);
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
      expect(useTranscriptStore.getState().segments[0]?.translation).toBe('こんにちは');
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: '1', translation: 'こんにちは' },
    ]);

    await translationService.translateCurrentTranscript();

    expect(invoke).toHaveBeenCalledWith('translate_transcript_segments', {
      request: {
        taskId: 'translate-task-id',
        config: expect.objectContaining({ apiKey: 'test-key', temperature: 0.7 }),
        segments: [{ id: '1', text: 'hello' }],
        targetLanguage: 'ja',
      },
    });
    expect(useTranscriptStore.getState().segments[0]?.translation).toBe('こんにちは');
    expect(useTranscriptStore.getState().getLlmState('current')).toEqual(expect.objectContaining({
      isTranslating: false,
      translationProgress: 100,
      isTranslationVisible: true,
    }));
  });

  it('falls back to final result when no chunk event arrives', async () => {
    useTranscriptStore.setState({
      config: buildTranslationTestConfig({
        translationLanguage: 'ja',
      }),
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      sourceHistoryId: null,
      llmStates: {
        current: {
          ...useTranscriptStore.getState().defaultLlmState,
          isTranslationVisible: true,
        },
      },
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', translation: 'こんにちは' }]);

    await translationService.translateCurrentTranscript();

    expect(useTranscriptStore.getState().segments[0]?.translation).toBe('こんにちは');
  });

  it('updates background history when the active record changes mid-translation', async () => {
    useTranscriptStore.setState({
      config: buildTranslationTestConfig({
        translationLanguage: 'zh',
      }),
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
      sourceHistoryId: 'history-a',
      llmStates: {
        'history-a': {
          ...useTranscriptStore.getState().defaultLlmState,
          isTranslationVisible: true,
        },
      },
    });

    mockListenToLlmTaskChunks.mockImplementation(async (_taskId, _taskType, onChunk) => {
      useTranscriptStore.setState({ sourceHistoryId: 'history-b' });
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
