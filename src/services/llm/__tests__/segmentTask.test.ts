import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../../../types/config';
import {
  applySegmentItemsToTranscriptJob,
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
} from '../segmentTask';

const mockCreateLlmTaskId = vi.fn();
const mockListenToLlmTaskChunks = vi.fn();
const mockListenToLlmTaskProgress = vi.fn();
const mockGetFeatureLlmConfig = vi.fn();
const mockIsLlmConfigComplete = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../../llmTaskService', () => ({
  createLlmTaskId: (...args: unknown[]) => mockCreateLlmTaskId(...args),
  listenToLlmTaskChunks: (...args: unknown[]) => mockListenToLlmTaskChunks(...args),
  listenToLlmTaskProgress: (...args: unknown[]) => mockListenToLlmTaskProgress(...args),
}));

vi.mock('../runtime', () => ({
  getFeatureLlmConfig: (...args: unknown[]) => mockGetFeatureLlmConfig(...args),
  isLlmConfigComplete: (...args: unknown[]) => mockIsLlmConfigComplete(...args),
}));

describe('segmentTask helpers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateLlmTaskId.mockReturnValue('shared-task-id');
    mockListenToLlmTaskChunks.mockResolvedValue(vi.fn());
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());
    mockGetFeatureLlmConfig.mockReturnValue({
      provider: 'open_ai',
      apiKey: 'test-key',
      model: 'test-model',
      baseUrl: 'https://example.com',
      temperature: 0.7,
    });
    mockIsLlmConfigComplete.mockReturnValue(true);
  });

  it('runConfiguredSegmentTask falls back to the final payload and cleans up the chunk listener', async () => {
    const unlistenChunk = vi.fn();
    mockListenToLlmTaskChunks.mockResolvedValue(unlistenChunk);
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: 'seg-1', translation: '你好' },
    ]);

    const onChunk = vi.fn();
    const config: Pick<AppConfig, 'llmSettings' | 'translationLanguage'> = {
      llmSettings: {
        activeProvider: 'open_ai',
        providers: {
          open_ai: {
            apiHost: 'https://example.com',
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
      },
      translationLanguage: 'zh',
    };

    const result = await runConfiguredSegmentTask({
      feature: 'translation',
      taskType: 'translate',
      config,
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'hello', isFinal: true }],
      onChunk,
      buildRequest: ({ taskId, llmConfig, segments, config }) => ({
        taskId,
        config: llmConfig,
        segments: segments.map(({ id, text }) => ({ id, text })),
        targetLanguage: config.translationLanguage || 'zh',
      }),
    });

    expect(result).toEqual([{ id: 'seg-1', translation: '你好' }]);
    expect(onChunk).toHaveBeenCalledWith([{ id: 'seg-1', translation: '你好' }]);
    expect(unlistenChunk).toHaveBeenCalledTimes(1);
  });

  it('runTranscriptSegmentTaskJob wires progress hooks, captured history id, and cleanup', async () => {
    const unlistenProgress = vi.fn();
    mockListenToLlmTaskProgress.mockImplementation(async (_taskId, _taskType, onProgress) => {
      onProgress({
        taskId: 'shared-task-id',
        taskType: 'polish',
        completedChunks: 1,
        totalChunks: 2,
      });
      return unlistenProgress;
    });

    const onStart = vi.fn();
    const onProgress = vi.fn();
    const runTask = vi.fn().mockResolvedValue(undefined);
    const onSuccess = vi.fn();
    const onFinally = vi.fn();

    await runTranscriptSegmentTaskJob({
      taskType: 'polish',
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'hello', isFinal: true }],
      sourceHistoryId: null,
      onStart,
      onProgress,
      runTask,
      onSuccess,
      onFinally,
    });

    expect(onStart).toHaveBeenCalledWith('current');
    expect(onProgress).toHaveBeenCalledWith(50, 'current');
    expect(runTask).toHaveBeenCalledWith('shared-task-id', 'current');
    expect(onSuccess).toHaveBeenCalledWith('current');
    expect(unlistenProgress).toHaveBeenCalledTimes(1);
    expect(onFinally).toHaveBeenCalledWith('current');
  });

  it('applySegmentItemsToTranscriptJob patches the original background history record after navigation changes', async () => {
    const applyToCurrentTranscript = vi.fn();
    const updateTranscript = vi.fn().mockResolvedValue(undefined);
    const loadTranscript = vi.fn().mockResolvedValue([
      { id: 'seg-1', start: 0, end: 1, text: 'hello', isFinal: true },
    ]);

    await applySegmentItemsToTranscriptJob({
      jobHistoryId: 'history-a',
      items: [{ id: 'seg-1', text: 'Hello' }],
      logLabel: 'PolishService',
      getCurrentHistoryId: () => 'history-b',
      applyToCurrentTranscript,
      loadTranscript,
      updateTranscript,
      mergeIntoSegments: (segments, items) => segments.map((segment) => (
        segment.id === items[0].id ? { ...segment, text: items[0].text } : segment
      )),
    });

    expect(applyToCurrentTranscript).not.toHaveBeenCalled();
    expect(loadTranscript).toHaveBeenCalledWith('history-a.json');
    expect(updateTranscript).toHaveBeenCalledWith('history-a', [
      { id: 'seg-1', start: 0, end: 1, text: 'Hello', isFinal: true },
    ]);
  });
});
