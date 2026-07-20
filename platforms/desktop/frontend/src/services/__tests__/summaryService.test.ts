import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { addLlmModel, createLlmSettings, setFeatureModelSelection, updateProviderSetting } from '../llm/state';
import { summaryService } from '../summaryService';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { buildTestConfig } from '../../test-utils/configTestUtils';

const mockCreateLlmTaskId = vi.fn();
const mockListenToLlmTaskProgress = vi.fn();
const mockListenToLlmTaskText = vi.fn();
const taskLedgerContext = vi.hoisted(() => ({
  upsertTaskLedgerRecord: vi.fn(),
  patchTaskLedgerRecord: vi.fn(),
  createLlmTaskLedgerId: vi.fn((taskId: string) => `llm-${taskId}`),
  isTaskLedgerCancelRequested: vi.fn(() => false),
  buildLlmTaskLedgerRecord: vi.fn((input: any) => ({
    id: `llm-${input.taskId}`,
    kind: 'llmSummary',
    status: 'running',
    title: 'AI Summary',
    progress: 0,
    createdAt: 100,
    updatedAt: 100,
    retryable: true,
    cancelable: true,
    recoverable: false,
    historyId: input.jobHistoryId === 'current' ? undefined : input.jobHistoryId,
    templateId: input.templateId,
  })),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../llmTaskTypes', async () => {
  const actual = await vi.importActual<typeof import('../llmTaskTypes')>('../llmTaskTypes');
  return {
    ...actual,
    createLlmTaskId: (...args: unknown[]) => mockCreateLlmTaskId(...args),
  };
});

vi.mock('../llmTaskEvents', () => ({
  listenToLlmTaskProgress: (...args: unknown[]) => mockListenToLlmTaskProgress(...args),
  listenToLlmTaskText: (...args: unknown[]) => mockListenToLlmTaskText(...args),
}));

vi.mock('../historyService', () => ({
  historyService: {
    loadSummary: vi.fn(),
    saveSummary: vi.fn(),
    deleteSummary: vi.fn(),
  },
}));

vi.mock('../taskLedgerBuilders', () => ({
  buildLlmTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(taskLedgerContext.buildLlmTaskLedgerRecord, undefined, args),
  createLlmTaskLedgerId: (...args: unknown[]) => Reflect.apply(taskLedgerContext.createLlmTaskLedgerId, undefined, args),
  isTaskLedgerCancelRequested: (...args: unknown[]) => Reflect.apply(taskLedgerContext.isTaskLedgerCancelRequested, undefined, args),
  patchTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(taskLedgerContext.patchTaskLedgerRecord, undefined, args),
  upsertTaskLedgerRecord: (...args: unknown[]) => Reflect.apply(taskLedgerContext.upsertTaskLedgerRecord, undefined, args),
}));

function createSummaryReadyConfig() {
  let llmSettings = createLlmSettings('open_ai');
  llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
    apiHost: 'https://api.openai.com',
    apiKey: 'test-key',
  });
  llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
  llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);

  return buildTestConfig({
    llmSettings,
  });
}

describe('summaryService', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    mockCreateLlmTaskId.mockReturnValue('summary-task-id');
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());
    mockListenToLlmTaskText.mockResolvedValue(vi.fn());
    taskLedgerContext.isTaskLedgerCancelRequested.mockReturnValue(false);

    useTranscriptStore.setState({
      segments: [],
      sourceHistoryId: null,
      summaryStates: {},
      config: createSummaryReadyConfig(),
    });
  });

  it('builds the unified summary job request and applies history-backed results', async () => {
    const { historyService } = await import('../historyService');
    const segments = [
      { id: '1', text: 'Hello world', start: 0, end: 2, isFinal: true },
      { id: '2', text: 'Next point', start: 2, end: 4, isFinal: true },
    ];

    useTranscriptStore.setState({
      segments,
      sourceHistoryId: 'history-a',
    });

    mockListenToLlmTaskProgress.mockImplementation(async (_taskId, _taskType, onProgress) => {
      onProgress({ taskId: 'summary-task-id', taskType: 'summary', completedChunks: 1, totalChunks: 2 });
      return vi.fn();
    });
    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      await onText({ taskId: 'summary-task-id', taskType: 'summary', text: 'Meeting stream', delta: 'Meeting stream' });
      return vi.fn();
    });
    vi.mocked(invoke).mockResolvedValue({
      taskId: 'summary-task-id',
      taskType: 'summary',
      jobHistoryId: 'history-a',
      summary: {
        activeTemplateId: 'meeting',
        record: {
          templateId: 'meeting',
          content: 'Meeting summary',
          generatedAt: '2026-05-04T00:00:00.000Z',
          sourceFingerprint: 'rust-fingerprint',
        },
      },
    });

    await summaryService.generateSummary('meeting');

    expect(invoke).toHaveBeenCalledWith('run_transcript_llm_job', {
      request: expect.objectContaining({
        taskId: 'summary-task-id',
        taskType: 'summary',
        jobHistoryId: 'history-a',
        template: {
          id: 'meeting',
          name: 'Meeting',
          instructions: expect.stringContaining('Meeting overview.'),
        },
        config: expect.objectContaining({
          apiKey: 'test-key',
          model: 'gpt-4o-mini',
          temperature: 0.7,
        }),
        segments: segments.map((segment) => expect.objectContaining(segment)),
      }),
    });
    expect(historyService.saveSummary).not.toHaveBeenCalled();
    expect(useTranscriptStore.getState().getSummaryState('history-a').record?.content).toBe('Meeting summary');
    expect(useTranscriptStore.getState().getSummaryState('history-a').record?.generatedAt).toBe('2026-05-04T00:00:00.000Z');
    expect(useTranscriptStore.getState().getSummaryState('history-a').record?.sourceFingerprint).toBe('rust-fingerprint');
    expect(useTranscriptStore.getState().getSummaryState('history-a').streamingContent).toBeUndefined();
    expect(useTranscriptStore.getState().getSummaryState('history-a').isGenerating).toBe(false);
  });

  it('updates temporary streaming content before the final summary record is written', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Live summary text', start: 0, end: 2, isFinal: true },
      ],
      sourceHistoryId: null,
    });

    let invokeResolved = false;
    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      await onText({ taskId: 'summary-task-id', taskType: 'summary', text: 'Partial summary', delta: 'Partial ' });
      expect(useTranscriptStore.getState().getSummaryState('current').streamingContent).toBe('Partial summary');
      expect(useTranscriptStore.getState().getSummaryState('current').record).toBeUndefined();
      return vi.fn();
    });
    vi.mocked(invoke).mockImplementation(async () => {
      invokeResolved = true;
      return {
        taskId: 'summary-task-id',
        taskType: 'summary',
        jobHistoryId: null,
        summary: {
          activeTemplateId: 'general',
          record: {
            templateId: 'general',
            content: 'Final summary',
            generatedAt: '2026-05-04T00:00:00.000Z',
            sourceFingerprint: 'rust-fingerprint',
          },
        },
      };
    });

    await summaryService.generateSummary('general');

    expect(invokeResolved).toBe(true);
    expect(useTranscriptStore.getState().getSummaryState('current').record?.content).toBe('Final summary');
    expect(useTranscriptStore.getState().getSummaryState('current').streamingContent).toBeUndefined();
  });

  it('keeps streamed summary text in memory when generation fails', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Live summary text', start: 0, end: 2, isFinal: true },
      ],
      sourceHistoryId: null,
    });

    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      await onText({ taskId: 'summary-task-id', taskType: 'summary', text: 'Recoverable partial', delta: 'Recoverable partial' });
      return vi.fn();
    });
    vi.mocked(invoke).mockRejectedValue(new Error('network failed'));

    await expect(summaryService.generateSummary('general')).rejects.toThrow('network failed');

    expect(useTranscriptStore.getState().getSummaryState('current').record).toBeUndefined();
    expect(useTranscriptStore.getState().getSummaryState('current').streamingContent).toBe('Recoverable partial');
    expect(useTranscriptStore.getState().getSummaryState('current').isGenerating).toBe(false);
  });

  it('moves a current summary result onto the new history id after save', async () => {
    const { historyService } = await import('../historyService');

    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Unsaved transcript', start: 0, end: 2, isFinal: true },
      ],
      sourceHistoryId: null,
    });

    vi.mocked(invoke).mockImplementation(async () => {
      useTranscriptStore.getState().setSourceHistoryId('history-new');
      return {
        taskId: 'summary-task-id',
        taskType: 'summary',
        jobHistoryId: null,
        summary: {
          activeTemplateId: 'general',
          record: {
            templateId: 'general',
            content: 'Current summary',
            generatedAt: '2026-05-04T00:00:00.000Z',
            sourceFingerprint: 'rust-fingerprint',
          },
        },
      };
    });

    await summaryService.generateSummary('general');

    expect(historyService.saveSummary).toHaveBeenCalledWith(
      'history-new',
      expect.objectContaining({
        record: expect.objectContaining({
          content: 'Current summary',
        }),
      }),
    );
    expect(useTranscriptStore.getState().summaryStates.current).toBeUndefined();
    expect(useTranscriptStore.getState().getSummaryState('history-new').record?.content).toBe('Current summary');
  });

  it('hydrates summary sidecars into the store', async () => {
    const { historyService } = await import('../historyService');
    vi.mocked(historyService.loadSummary).mockResolvedValue({
      activeTemplateId: 'lecture',
      record: {
        templateId: 'lecture',
        content: 'Lecture summary',
        generatedAt: '2026-04-22T10:00:00.000Z',
        sourceFingerprint: 'fingerprint-a',
      },
    });

    await summaryService.loadSummary('history-lecture');

    expect(useTranscriptStore.getState().getSummaryState('history-lecture')).toEqual(expect.objectContaining({
      activeTemplateId: 'lecture',
      record: expect.objectContaining({
        content: 'Lecture summary',
      }),
    }));
  });

  it('creates the first manual summary record when none exists yet', async () => {
    const { historyService } = await import('../historyService');
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Manual transcript', start: 0, end: 2, isFinal: true },
      ],
      sourceHistoryId: 'history-manual',
      summaryStates: {
        'history-manual': {
          activeTemplateId: 'meeting',
          record: undefined,
          isGenerating: false,
          generationProgress: 0,
        },
      },
    });

    await summaryService.updateSummaryRecord('Manual summary', 'history-manual');

    expect(useTranscriptStore.getState().getSummaryState('history-manual').record).toEqual(
      expect.objectContaining({
        templateId: 'meeting',
        content: 'Manual summary',
      }),
    );
    expect(historyService.saveSummary).toHaveBeenCalledWith(
      'history-manual',
      expect.objectContaining({
        activeTemplateId: 'meeting',
        record: expect.objectContaining({
          templateId: 'meeting',
          content: 'Manual summary',
        }),
      }),
    );
  });

  it('rejects new summary generation when Summary is disabled', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 2, isFinal: true },
      ],
      config: {
        ...createSummaryReadyConfig(),
        summaryEnabled: false,
      },
    });
    vi.mocked(invoke).mockClear();

    await expect(summaryService.generateSummary('general')).rejects.toThrow('Summary is disabled.');
    expect(invoke).not.toHaveBeenCalledWith('run_transcript_llm_job', expect.anything());
  });

  it('rejects new summary generation when the summary config is incomplete', async () => {
    const readyConfig = createSummaryReadyConfig();

    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 2, isFinal: true },
      ],
      config: {
        ...readyConfig,
        llmSettings: updateProviderSetting(readyConfig.llmSettings, 'open_ai', {
          apiHost: 'https://api.openai.com',
          apiKey: '',
        }),
      },
    });
    vi.mocked(invoke).mockClear();

    await expect(summaryService.generateSummary('general')).rejects.toThrow('LLM Service not fully configured.');
    expect(invoke).not.toHaveBeenCalledWith('run_transcript_llm_job', expect.anything());
  });

  it('retries summary generation with the ledger template id and explicit history segments', async () => {
    const segments = [
      { id: '1', text: 'Retry summary text', start: 0, end: 2, isFinal: true },
    ];
    useTranscriptStore.setState({
      segments: [],
      sourceHistoryId: null,
    });
    vi.mocked(invoke).mockResolvedValue({
      taskId: 'summary-task-id',
      taskType: 'summary',
      jobHistoryId: 'history-retry',
      summary: {
        activeTemplateId: 'meeting',
        record: {
          templateId: 'meeting',
          content: 'Retried summary',
          generatedAt: '2026-05-05T00:00:00.000Z',
          sourceFingerprint: 'retry-fingerprint',
        },
      },
    });

    await summaryService.retrySummaryTranscriptJob({
      segments,
      historyId: 'history-retry',
      templateId: 'meeting',
    });

    expect(invoke).toHaveBeenCalledWith('run_transcript_llm_job', {
      request: expect.objectContaining({
        taskId: 'summary-task-id',
        taskType: 'summary',
        jobHistoryId: 'history-retry',
        template: expect.objectContaining({ id: 'meeting' }),
        segments,
      }),
    });
    expect(useTranscriptStore.getState().getSummaryState('history-retry').record?.content).toBe('Retried summary');
  });
});
