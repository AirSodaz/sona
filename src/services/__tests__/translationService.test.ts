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
const mockListenToTranscriptLlmJobUpdates = vi.fn();
const mockCreateLlmTaskId = vi.fn();
const mockCreateSnapshot = vi.fn();

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

vi.mock('../llmTaskTypes', () => ({
  createLlmTaskId: (...args: unknown[]) => mockCreateLlmTaskId(...args),
}));

vi.mock('../llmTaskEvents', () => ({
  listenToLlmTaskChunks: (...args: unknown[]) => mockListenToLlmTaskChunks(...args),
  listenToLlmTaskProgress: (...args: unknown[]) => mockListenToLlmTaskProgress(...args),
  listenToTranscriptLlmJobUpdates: (...args: unknown[]) => mockListenToTranscriptLlmJobUpdates(...args),
}));

vi.mock('../transcriptSnapshotService', () => ({
  transcriptSnapshotService: {
    createSnapshot: (...args: unknown[]) => mockCreateSnapshot(...args),
  },
}));

vi.mock('../historyService', () => ({
  historyService: {
    createTranscriptSnapshot: vi.fn(),
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
    mockListenToTranscriptLlmJobUpdates.mockResolvedValue(vi.fn());
    mockCreateSnapshot.mockResolvedValue(null);
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
    mockListenToTranscriptLlmJobUpdates.mockImplementation(async (_taskId, _taskType, onUpdate) => {
      await onUpdate({
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: null,
        segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true, translation: 'こんにちは' }],
      });
      expect(useTranscriptStore.getState().segments[0]?.translation).toBe('こんにちは');
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'translate-task-id',
      taskType: 'translate',
      jobHistoryId: null,
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true, translation: 'こんにちは' }],
    });

    await translationService.translateCurrentTranscript();

    expect(invoke).toHaveBeenCalledWith('run_transcript_llm_job', {
      request: {
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: null,
        config: expect.objectContaining({ apiKey: 'test-key', temperature: 0.7 }),
        segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }],
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
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'translate-task-id',
      taskType: 'translate',
      jobHistoryId: null,
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true, translation: 'こんにちは' }],
    });

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

    mockListenToTranscriptLlmJobUpdates.mockImplementation(async (_taskId, _taskType, onUpdate) => {
      useTranscriptStore.setState({ sourceHistoryId: 'history-b' });
      await onUpdate({
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: 'history-a',
        segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true, translation: '浣犲ソ' }],
      });
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'translate-task-id',
      taskType: 'translate',
      jobHistoryId: 'history-a',
      segments: [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true, translation: '浣犲ソ' }],
    });

    await translationService.translateCurrentTranscript();

    expect(invoke).toHaveBeenCalledWith('run_transcript_llm_job', {
      request: expect.objectContaining({
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: 'history-a',
      }),
    });
    expect(historyService.loadTranscript).not.toHaveBeenCalled();
    expect(historyService.updateTranscript).not.toHaveBeenCalled();
    expect(mockCreateSnapshot).not.toHaveBeenCalled();
  });

  it('retries a saved transcript translation with the ledger target language', async () => {
    const segments = [{ id: '1', start: 0, end: 1, text: 'hello', isFinal: true }];
    useTranscriptStore.setState({
      config: buildTranslationTestConfig({
        translationLanguage: 'zh',
      }),
      segments: [],
      sourceHistoryId: null,
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'translate-task-id',
      taskType: 'translate',
      jobHistoryId: 'history-retry',
      segments: [{ ...segments[0], translation: 'こんにちは' }],
    });

    await translationService.retryTranslateTranscriptJob({
      segments,
      historyId: 'history-retry',
      targetLanguage: 'ja',
    });

    expect(invoke).toHaveBeenCalledWith('run_transcript_llm_job', {
      request: expect.objectContaining({
        taskId: 'translate-task-id',
        taskType: 'translate',
        jobHistoryId: 'history-retry',
        segments,
        targetLanguage: 'ja',
      }),
    });
    expect(historyService.updateTranscript).not.toHaveBeenCalled();
  });
});
