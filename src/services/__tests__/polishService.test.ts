import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { polishService } from '../polishService';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';
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

describe('PolishService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTranscriptStores();
    mockCreateLlmTaskId.mockReturnValue('polish-task-id');
    mockListenToLlmTaskChunks.mockResolvedValue(vi.fn());
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());

    useTranscriptStore.setState({
      config: {
        llmSettings: {
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
            polishModelId: 'open-ai-test',
          },
        },
        polishPresetId: 'general',
        polishCustomPresets: [],
        polishKeywords: '',
        polishKeywordSets: [],
      },
      segments: [],
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
        config: expect.objectContaining({ apiKey: 'test-key', temperature: 0.7 }),
        segments: [
          { id: '1', text: 'hello' },
          { id: '2', text: 'world' },
        ],
        context: '',
        keywords: '',
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

  it('polishSegments resolves custom preset context before invoking Rust', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    useTranscriptStore.setState({
      config: {
        llmSettings: {
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
            polishModelId: 'open-ai-test',
          },
        },
        polishPresetId: 'custom-team',
        polishCustomPresets: [
          { id: 'custom-team', name: 'Team', context: 'Team sync notes' },
        ],
        polishKeywords: '',
        polishKeywordSets: [],
      },
    });

    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', text: 'Hello' }]);

    await polishService.polishSegments(segments);

    expect(invoke).toHaveBeenCalledWith('polish_transcript_segments', {
      request: expect.objectContaining({
        context: 'Team sync notes',
      }),
    });
  });

  it('polishSegments resolves enabled keyword set blocks before invoking Rust', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    useTranscriptStore.setState({
      config: {
        llmSettings: {
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
            polishModelId: 'open-ai-test',
          },
        },
        polishPresetId: 'general',
        polishCustomPresets: [],
        polishKeywords: '',
        polishKeywordSets: [
          { id: 'kw-1', name: 'Brand', enabled: true, keywords: 'Sona\nSherpa-onnx' },
          { id: 'kw-2', name: 'Disabled', enabled: false, keywords: 'Ignore me' },
          { id: 'kw-3', name: 'Style', enabled: true, keywords: 'Preserve speaker names' },
        ],
      },
    });

    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', text: 'Hello' }]);

    await polishService.polishSegments(segments);

    expect(invoke).toHaveBeenCalledWith('polish_transcript_segments', {
      request: expect.objectContaining({
        keywords: 'Sona\nSherpa-onnx\n\nPreserve speaker names',
      }),
    });
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

    useTranscriptStore.setState({
      segments,
      sourceHistoryId: null,
    });
    mockListenToLlmTaskProgress.mockImplementation(async (_taskId, _taskType, onProgress) => {
      onProgress({ taskId: 'polish-task-id', taskType: 'polish', completedChunks: 1, totalChunks: 2 });
      expect(useTranscriptStore.getState().getLlmState('current').polishProgress).toBe(50);
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
      expect(useTranscriptStore.getState().segments[0]?.text).toBe('Hello');
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
    expect(useTranscriptStore.getState().segments[0]?.text).toBe('Hello');
    expect(useTranscriptStore.getState().getLlmState('current')).toEqual(expect.objectContaining({
      isPolishing: false,
      polishProgress: 0,
    }));
  });
});
