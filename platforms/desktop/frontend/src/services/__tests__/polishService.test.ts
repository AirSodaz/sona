import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_SESSION_DATA,
  useTranscriptStore as useCanonicalTranscriptStore,
} from '../../stores/transcriptStore';
import {
  buildTestConfig as buildBaseTestConfig,
  type DeepPartial,
} from '../../test-utils/configTestUtils';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';
import type { AppConfig } from '../../types/config';
import type { TranscriptSegment } from '../../types/transcript';
import { polishService } from '../polishService';

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
    polishModelId: 'open-ai-test',
  },
} satisfies NonNullable<AppConfig['llmSettings']>;

type TestConfigOverrides = DeepPartial<AppConfig>;

function buildPolishTestConfig(overrides: TestConfigOverrides = {}): AppConfig {
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
  listenToTranscriptLlmJobUpdates: (...args: unknown[]) =>
    mockListenToTranscriptLlmJobUpdates(...args),
}));

vi.mock('../transcriptSnapshotService', () => ({
  transcriptSnapshotService: {
    createSnapshot: (...args: unknown[]) => mockCreateSnapshot(...args),
  },
}));

describe('PolishService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTranscriptStores();
    mockCreateLlmTaskId.mockReturnValue('polish-task-id');
    mockListenToLlmTaskChunks.mockResolvedValue(vi.fn());
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());
    mockListenToTranscriptLlmJobUpdates.mockResolvedValue(vi.fn());
    mockCreateSnapshot.mockResolvedValue(null);

    useTranscriptStore.setState({
      config: buildPolishTestConfig(),
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
      request: expect.objectContaining({
        taskId: 'polish-task-id',
        config: expect.objectContaining({ apiKey: 'test-key', temperature: 0.7 }),
        segments: [
          { id: '1', text: 'hello' },
          { id: '2', text: 'world' },
        ],
        mode: 'clean',
      }),
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
      config: buildPolishTestConfig({
        polishPresetId: 'custom-team',
        polishCustomPresets: [{ id: 'custom-team', name: 'Team', context: 'Team sync notes' }],
      }),
    });

    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', text: 'Hello' }]);

    await polishService.polishSegments(segments);

    expect(invoke).toHaveBeenCalledWith('polish_transcript_segments', {
      request: expect.objectContaining({
        context: 'Team sync notes',
      }),
    });
  });

  it('polishSegments resolves directive modes before invoking Rust', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    useTranscriptStore.setState({
      config: buildPolishTestConfig({
        polishPresetId: 'verbatim',
      }),
    });

    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue([{ id: '1', text: 'Hello' }]);

    await polishService.polishSegments(segments);

    expect(invoke).toHaveBeenCalledWith('polish_transcript_segments', {
      request: expect.objectContaining({
        mode: 'verbatim',
      }),
    });
  });

  it('polishSegments surfaces normalized Rust command errors', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    (invoke as unknown as ReturnType<typeof vi.fn>).mockRejectedValue(
      'error invoking command `polish_transcript_segments`: failed to deserialize response body: Caused by: LLM Error'
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
      onProgress({
        taskId: 'polish-task-id',
        taskType: 'polish',
        completedChunks: 1,
        totalChunks: 2,
      });
      expect(useTranscriptStore.getState().getLlmState('current').polishProgress).toBe(50);
      return vi.fn();
    });
    mockListenToTranscriptLlmJobUpdates.mockImplementation(async (_taskId, _taskType, onUpdate) => {
      await onUpdate({
        taskId: 'polish-task-id',
        taskType: 'polish',
        jobHistoryId: null,
        segments: [{ id: '1', start: 0, end: 1, text: 'Hello', isFinal: true }],
      });
      expect(useTranscriptStore.getState().segments[0]?.text).toBe('Hello');
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'polish-task-id',
      taskType: 'polish',
      jobHistoryId: null,
      segments: [{ id: '1', start: 0, end: 1, text: 'Hello', isFinal: true }],
    });

    await polishService.polishTranscript();

    expect(mockListenToLlmTaskProgress).toHaveBeenCalledWith(
      'polish-task-id',
      'polish',
      expect.any(Function)
    );
    expect(mockListenToTranscriptLlmJobUpdates).toHaveBeenCalledWith(
      'polish-task-id',
      'polish',
      expect.any(Function)
    );
    expect(useTranscriptStore.getState().segments[0]?.text).toBe('Hello');
    expect(useTranscriptStore.getState().getLlmState('current')).toEqual(
      expect.objectContaining({
        isPolishing: false,
        polishProgress: 0,
      })
    );
  });

  it('delegates saved-history snapshot and writeback to the Rust transcript job', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    useTranscriptStore.setState({
      segments,
      sourceHistoryId: 'history-a',
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'polish-task-id',
      taskType: 'polish',
      jobHistoryId: 'history-a',
      segments: [{ id: '1', start: 0, end: 1, text: 'Hello', isFinal: true }],
    });

    await polishService.polishTranscript();

    expect(invoke).toHaveBeenCalledWith('run_transcript_llm_job', {
      request: expect.objectContaining({
        taskId: 'polish-task-id',
        taskType: 'polish',
        jobHistoryId: 'history-a',
      }),
    });
    expect(mockCreateSnapshot).not.toHaveBeenCalled();
  });

  it('retries a saved transcript polish job through the Rust transcript job', async () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
    ];

    useTranscriptStore.setState({
      segments: [],
      sourceHistoryId: null,
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'polish-task-id',
      taskType: 'polish',
      jobHistoryId: 'history-retry',
      segments: [{ id: '1', start: 0, end: 1, text: 'Hello', isFinal: true }],
    });

    await polishService.retryPolishTranscriptJob({
      segments,
      historyId: 'history-retry',
    });

    expect(invoke).toHaveBeenCalledWith('run_transcript_llm_job', {
      request: expect.objectContaining({
        taskId: 'polish-task-id',
        taskType: 'polish',
        jobHistoryId: 'history-retry',
        segments,
      }),
    });
    expect(mockCreateSnapshot).not.toHaveBeenCalled();
  });

  it('applies polish updates to the originating session when user switches tabs mid-job', async () => {
    const originalSegments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1, text: 'hello world', isFinal: true },
    ];
    const activeCreateSegments: TranscriptSegment[] = [
      { id: '2', start: 0, end: 1, text: 'create tab', isFinal: true },
    ];

    useCanonicalTranscriptStore.setState({
      activeSessionId: 'history-a',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'history-a': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'history-a',
          segments: originalSegments,
        },
        'create-b': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: 'create-b',
          segments: activeCreateSegments,
        },
      },
    });

    mockListenToTranscriptLlmJobUpdates.mockImplementation(async (_taskId, _taskType, onUpdate) => {
      useCanonicalTranscriptStore.getState().activateSession('create-b');
      await onUpdate({
        taskId: 'polish-task-id',
        taskType: 'polish',
        jobHistoryId: 'history-a',
        segments: [{ id: '1', start: 0, end: 1, text: 'Hello, world.', isFinal: true }],
      });
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'polish-task-id',
      taskType: 'polish',
      jobHistoryId: 'history-a',
      segments: [{ id: '1', start: 0, end: 1, text: 'Hello, world.', isFinal: true }],
    });

    await polishService.polishTranscript();

    expect(useCanonicalTranscriptStore.getState().sessions['history-a'].segments[0].text).toBe(
      'Hello, world.'
    );
    expect(useCanonicalTranscriptStore.getState().sessions['create-b'].segments[0].text).toBe(
      'create tab'
    );
  });

  it('targets the originating unsaved session even if user switches to another unsaved session mid-job', async () => {
    useCanonicalTranscriptStore.setState({
      activeSessionId: 'unsaved-a',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        'unsaved-a': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: null,
          segments: [{ id: '1', start: 0, end: 1, text: 'raw a', isFinal: true }],
        },
        'unsaved-b': {
          ...DEFAULT_SESSION_DATA,
          sourceHistoryId: null,
          segments: [{ id: '2', start: 0, end: 1, text: 'raw b', isFinal: true }],
        },
      },
    });

    mockListenToTranscriptLlmJobUpdates.mockImplementation(async (_taskId, _taskType, onUpdate) => {
      useCanonicalTranscriptStore.getState().activateSession('unsaved-b');
      await onUpdate({
        taskId: 'polish-task-id',
        taskType: 'polish',
        jobHistoryId: null,
        segments: [{ id: '1', start: 0, end: 1, text: 'Polished A', isFinal: true }],
      });
      return vi.fn();
    });
    (invoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      taskId: 'polish-task-id',
      taskType: 'polish',
      jobHistoryId: null,
      segments: [{ id: '1', start: 0, end: 1, text: 'Polished A', isFinal: true }],
    });

    await polishService.polishTranscript();

    expect(useCanonicalTranscriptStore.getState().sessions['unsaved-a'].segments[0].text).toBe(
      'Polished A'
    );
    expect(useCanonicalTranscriptStore.getState().sessions['unsaved-b'].segments[0].text).toBe(
      'raw b'
    );
  });
});
