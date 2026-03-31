import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { polishService } from '../polishService';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { TranscriptSegment } from '../../types/transcript';

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

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: {
    getState: vi.fn(),
    setState: vi.fn(),
    subscribe: vi.fn(),
  },
}));

describe('PolishService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCreateLlmTaskId.mockReturnValue('polish-task-id');
    mockListenToLlmTaskChunks.mockResolvedValue(vi.fn());
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());

    (useTranscriptStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue({
      config: {
        llm: {
          provider: 'open_ai',
          baseUrl: 'test-url',
          apiKey: 'test-key',
          model: 'test-model',
          temperature: 0.7,
        },
        polishScenario: 'custom',
        polishContext: '',
        polishKeywords: '',
      },
      segments: [],
      updateSegment: vi.fn(),
      updateLlmState: vi.fn(),
      sourceHistoryId: null,
    });
  });

  it('polishSegments forwards chunk events immediately', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
      { id: '2', start: 1, end: 2, text: 'world', isFinal: true },
    ];

    mockListenToLlmTaskChunks.mockImplementation(async (_taskId, _taskType, onChunk) => {
      await onChunk({
        taskId: 'polish-task-id',
        taskType: 'polish',
        chunkIndex: 1,
        totalChunks: 1,
        items: [
          { id: '1', text: 'Hello' },
          { id: '2', text: 'World' },
        ],
      });
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: '1', text: 'Hello' },
      { id: '2', text: 'World' },
    ]);

    const onChunk = vi.fn();

    await polishService.polishSegments(segments, onChunk);

    expect(invoke).toHaveBeenCalledWith('polish_transcript_segments', {
      request: {
        taskId: 'polish-task-id',
        config: expect.objectContaining({ apiKey: 'test-key' }),
        segments: [
          { id: '1', text: 'hello' },
          { id: '2', text: 'world' },
        ],
        context: '',
        keywords: '',
        scenarioPrompt: '',
      },
    });
    expect(onChunk).toHaveBeenCalledTimes(1);
    expect(onChunk).toHaveBeenCalledWith([
      { id: '1', text: 'Hello' },
      { id: '2', text: 'World' },
    ]);
  });

  it('polishSegments falls back to final result when no chunk event arrives', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];
    const onChunk = vi.fn();

    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', text: 'Hello' }]);

    await polishService.polishSegments(segments, onChunk);

    expect(onChunk).toHaveBeenCalledWith([{ id: '1', text: 'Hello' }]);
  });

  it('polishSegments surfaces normalized Rust command errors', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    (invoke as unknown as ReturnType<typeof vi.fn>).mockRejectedValue(
      'error invoking command `polish_transcript_segments`: failed to deserialize response body: Caused by: LLM Error',
    );

    await expect(polishService.polishSegments(segments)).rejects.toThrow('LLM Error');
  });

  it('polishTranscript applies chunk events and updates progress', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    const mockStore = {
      config: {
        llm: {
          provider: 'open_ai',
          baseUrl: 'test-url',
          apiKey: 'test-key',
          model: 'test-model',
          temperature: 0.7,
        },
        polishScenario: 'custom',
        polishContext: '',
        polishKeywords: '',
      },
      segments,
      updateSegment: vi.fn(),
      updateLlmState: vi.fn(),
      sourceHistoryId: null,
    };

    (useTranscriptStore.getState as unknown as ReturnType<typeof vi.fn>).mockReturnValue(mockStore);
    mockListenToLlmTaskProgress.mockImplementation(async (_taskId, _taskType, onProgress) => {
      onProgress({ taskId: 'polish-task-id', taskType: 'polish', completedChunks: 1, totalChunks: 2 });
      return vi.fn();
    });
    mockListenToLlmTaskChunks.mockImplementation(async (_taskId, _taskType, onChunk) => {
      await onChunk({
        taskId: 'polish-task-id',
        taskType: 'polish',
        chunkIndex: 1,
        totalChunks: 2,
        items: [{ id: '1', text: 'Hello' }],
      });
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', text: 'Hello' }]);

    await polishService.polishTranscript();

    expect(mockListenToLlmTaskProgress).toHaveBeenCalledWith(
      'polish-task-id',
      'polish',
      expect.any(Function),
    );
    expect(mockListenToLlmTaskChunks).toHaveBeenCalledWith(
      'polish-task-id',
      'polish',
      expect.any(Function),
    );
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ isPolishing: true, polishProgress: 0 }, 'current');
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ polishProgress: 50 }, 'current');
    expect(mockStore.updateSegment).toHaveBeenCalledWith('1', { text: 'Hello' });
    expect(mockStore.updateLlmState).toHaveBeenCalledWith({ isPolishing: false, polishProgress: 0 }, 'current');
  });
});
