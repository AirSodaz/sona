import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import {
  addLlmModel,
  createLlmSettings,
  setFeatureModelSelection,
  updateProviderSetting,
} from '../llm/state';
import { isSummaryRecordStale, summaryService } from '../summaryService';

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
  buildLlmTaskLedgerRecord: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.buildLlmTaskLedgerRecord, undefined, args),
  createLlmTaskLedgerId: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.createLlmTaskLedgerId, undefined, args),
  isTaskLedgerCancelRequested: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.isTaskLedgerCancelRequested, undefined, args),
  patchTaskLedgerRecord: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.patchTaskLedgerRecord, undefined, args),
  upsertTaskLedgerRecord: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.upsertTaskLedgerRecord, undefined, args),
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
      onProgress({
        taskId: 'summary-task-id',
        taskType: 'summary',
        completedChunks: 1,
        totalChunks: 2,
      });
      return vi.fn();
    });
    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      await onText({
        taskId: 'summary-task-id',
        taskType: 'summary',
        text: 'Meeting stream',
        delta: 'Meeting stream',
      });
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
    expect(useTranscriptStore.getState().getSummaryState('history-a').record?.content).toBe(
      'Meeting summary'
    );
    expect(useTranscriptStore.getState().getSummaryState('history-a').record?.generatedAt).toBe(
      '2026-05-04T00:00:00.000Z'
    );
    expect(
      useTranscriptStore.getState().getSummaryState('history-a').record?.sourceFingerprint
    ).toBe('rust-fingerprint');
    expect(
      useTranscriptStore.getState().getSummaryState('history-a').streamingContent
    ).toBeUndefined();
    expect(useTranscriptStore.getState().getSummaryState('history-a').isGenerating).toBe(false);
  });

  it('updates temporary streaming content before the final summary record is written', async () => {
    useTranscriptStore.setState({
      segments: [{ id: '1', text: 'Live summary text', start: 0, end: 2, isFinal: true }],
      sourceHistoryId: null,
    });

    let invokeResolved = false;
    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      await onText({
        taskId: 'summary-task-id',
        taskType: 'summary',
        text: 'Partial summary',
        delta: 'Partial ',
      });
      expect(useTranscriptStore.getState().getSummaryState('current').streamingContent).toBe(
        'Partial summary'
      );
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
    expect(useTranscriptStore.getState().getSummaryState('current').record?.content).toBe(
      'Final summary'
    );
    expect(
      useTranscriptStore.getState().getSummaryState('current').streamingContent
    ).toBeUndefined();
  });

  it('separates thought reasoning stream from content stream', async () => {
    useTranscriptStore.setState({
      segments: [{ id: '1', text: 'Live summary text', start: 0, end: 2, isFinal: true }],
      sourceHistoryId: null,
    });

    let invokeResolved = false;
    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      await onText({
        taskId: 'summary-task-id',
        taskType: 'summary',
        text: 'Thinking deeply...',
        delta: 'Thinking deeply...',
        reset: false,
        isThought: true,
      });
      expect(useTranscriptStore.getState().getSummaryState('current').streamingThought).toBe(
        'Thinking deeply...'
      );
      expect(useTranscriptStore.getState().getSummaryState('current').streamingContent).toBe('');

      await onText({
        taskId: 'summary-task-id',
        taskType: 'summary',
        text: 'Actual content',
        delta: 'Actual content',
        reset: false,
        isThought: false,
      });
      expect(useTranscriptStore.getState().getSummaryState('current').streamingThought).toBe(
        'Thinking deeply...'
      );
      expect(useTranscriptStore.getState().getSummaryState('current').streamingContent).toBe(
        'Actual content'
      );
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
            content: 'Actual content',
            generatedAt: '2026-03-30T00:00:00.000Z',
            sourceFingerprint: 'fingerprint-1',
          },
        },
      };
    });

    await summaryService.generateSummary('general');
    expect(invokeResolved).toBe(true);
  });

  it('keeps streamed summary text in memory when generation fails', async () => {
    useTranscriptStore.setState({
      segments: [{ id: '1', text: 'Live summary text', start: 0, end: 2, isFinal: true }],
      sourceHistoryId: null,
    });

    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      await onText({
        taskId: 'summary-task-id',
        taskType: 'summary',
        text: 'Recoverable partial',
        delta: 'Recoverable partial',
      });
      return vi.fn();
    });
    vi.mocked(invoke).mockRejectedValue(new Error('network failed'));

    await expect(summaryService.generateSummary('general')).rejects.toThrow('network failed');

    expect(useTranscriptStore.getState().getSummaryState('current').record).toBeUndefined();
    expect(useTranscriptStore.getState().getSummaryState('current').streamingContent).toBe(
      'Recoverable partial'
    );
    expect(useTranscriptStore.getState().getSummaryState('current').isGenerating).toBe(false);
  });

  it('moves a current summary result onto the new history id after save', async () => {
    const { historyService } = await import('../historyService');

    useTranscriptStore.setState({
      segments: [{ id: '1', text: 'Unsaved transcript', start: 0, end: 2, isFinal: true }],
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
      })
    );
    expect(useTranscriptStore.getState().summaryStates.current).toBeUndefined();
    expect(useTranscriptStore.getState().getSummaryState('history-new').record?.content).toBe(
      'Current summary'
    );
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

    expect(useTranscriptStore.getState().getSummaryState('history-lecture')).toEqual(
      expect.objectContaining({
        activeTemplateId: 'lecture',
        record: expect.objectContaining({
          content: 'Lecture summary',
        }),
      })
    );
  });

  it('creates the first manual summary record when none exists yet', async () => {
    const { historyService } = await import('../historyService');
    useTranscriptStore.setState({
      segments: [{ id: '1', text: 'Manual transcript', start: 0, end: 2, isFinal: true }],
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
      })
    );
    expect(historyService.saveSummary).toHaveBeenCalledWith(
      'history-manual',
      expect.objectContaining({
        activeTemplateId: 'meeting',
        record: expect.objectContaining({
          templateId: 'meeting',
          content: 'Manual summary',
        }),
      })
    );
  });

  it('rejects new summary generation when Summary is disabled', async () => {
    useTranscriptStore.setState({
      segments: [{ id: '1', text: 'Transcript text', start: 0, end: 2, isFinal: true }],
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
      segments: [{ id: '1', text: 'Transcript text', start: 0, end: 2, isFinal: true }],
      config: {
        ...readyConfig,
        llmSettings: updateProviderSetting(readyConfig.llmSettings, 'open_ai', {
          apiHost: 'https://api.openai.com',
          apiKey: '',
        }),
      },
    });
    vi.mocked(invoke).mockClear();

    await expect(summaryService.generateSummary('general')).rejects.toThrow(
      'LLM Service not fully configured.'
    );
    expect(invoke).not.toHaveBeenCalledWith('run_transcript_llm_job', expect.anything());
  });

  it('retries summary generation with the ledger template id and explicit history segments', async () => {
    const segments = [{ id: '1', text: 'Retry summary text', start: 0, end: 2, isFinal: true }];
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
    expect(useTranscriptStore.getState().getSummaryState('history-retry').record?.content).toBe(
      'Retried summary'
    );
  });
});

describe('isSummaryRecordStale', () => {
  it('returns false for matching current fingerprints', () => {
    const segments = [{ id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }];
    const record = {
      templateId: 'general',
      content: 'Summary',
      generatedAt: '2026-05-05T00:00:00.000Z',
      sourceFingerprint: '1:Hello:0:1:true:::',
    };
    expect(isSummaryRecordStale(record, segments)).toBe(false);
  });

  it('handles legacy fingerprints that included speaker score', () => {
    const segments = [
      {
        id: '1',
        text: 'Hello',
        start: 0,
        end: 1,
        isFinal: true,
        speaker: { id: 'spk-1', label: 'Alice', kind: 'identified' as const, score: 0.8492019672 },
      },
    ];
    const legacyRecordWithRustScore = {
      templateId: 'general',
      content: 'Summary',
      generatedAt: '2026-05-05T00:00:00.000Z',
      sourceFingerprint: '1:Hello:0:1:true:spk-1:Alice:identified:0.849202',
    };
    expect(isSummaryRecordStale(legacyRecordWithRustScore, segments)).toBe(false);
  });

  it('handles legacy fingerprints with empty speaker score', () => {
    const segments = [{ id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }];
    const legacyRecordWithEmptyScore = {
      templateId: 'general',
      content: 'Summary',
      generatedAt: '2026-05-05T00:00:00.000Z',
      sourceFingerprint: '1:Hello:0:1:true::::',
    };
    expect(isSummaryRecordStale(legacyRecordWithEmptyScore, segments)).toBe(false);
  });

  it('handles legacy fingerprints without speaker fields', () => {
    const segments = [{ id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }];
    const legacyRecordNoSpeaker = {
      templateId: 'general',
      content: 'Summary',
      generatedAt: '2026-05-05T00:00:00.000Z',
      sourceFingerprint: '1:Hello:0:1:true',
    };
    expect(isSummaryRecordStale(legacyRecordNoSpeaker, segments)).toBe(false);
  });

  it('returns true when transcript text has actually changed', () => {
    const segments = [{ id: '1', text: 'New text', start: 0, end: 1, isFinal: true }];
    const record = {
      templateId: 'general',
      content: 'Summary',
      generatedAt: '2026-05-05T00:00:00.000Z',
      sourceFingerprint: '1:Hello:0:1:true:::',
    };
    expect(isSummaryRecordStale(record, segments)).toBe(true);
  });

  it('isolates streaming and final summary writes to session:<id> when switching tabs mid-stream on an unpersisted draft', async () => {
    const { historyService } = await import('../historyService');
    const { useTranscriptSidecarStore } = await import('../../stores/transcriptSidecarStore');

    useTranscriptStore.setState({
      config: createSummaryReadyConfig(),
      activeSessionId: 'draft-rec-1',
      sessions: {
        default: { ...useTranscriptStore.getState().sessions.default },
        'draft-rec-1': {
          ...useTranscriptStore.getState().sessions.default,
          sourceHistoryId: null,
          segments: [{ id: 'd-1', text: 'Draft call transcript', start: 0, end: 2, isFinal: true }],
        },
        'history-other': {
          ...useTranscriptStore.getState().sessions.default,
          sourceHistoryId: 'history-other',
          segments: [{ id: 'o-1', text: 'Other history', start: 0, end: 2, isFinal: true }],
        },
      },
    });
    useTranscriptSidecarStore.getState().setSummaryState(
      {
        activeTemplateId: 'general',
        record: {
          templateId: 'general',
          content: 'Existing other summary',
          generatedAt: '2026-05-01T00:00:00.000Z',
          sourceFingerprint: 'other-fp',
        },
      },
      'history-other'
    );

    let emitChunk: ((event: any) => Promise<void>) | undefined;
    mockListenToLlmTaskText.mockImplementation(async (_taskId, _taskType, onText) => {
      emitChunk = onText;
      return vi.fn();
    });

    const invokeGate = Promise.withResolvers<any>();
    vi.mocked(invoke).mockImplementation(async () => invokeGate.promise);

    const jobPromise = summaryService.generateSummary({
      templateId: 'general',
      sessionId: 'draft-rec-1',
    });

    // Switch foreground tab to history-other while streaming
    await vi.waitFor(() => {
      expect(emitChunk).toBeDefined();
    });
    useTranscriptStore.getState().activateSession('history-other');

    await emitChunk?.({
      taskId: 'summary-task-id',
      taskType: 'summary',
      text: 'Draft partial stream',
      delta: 'Draft partial stream',
    });

    expect(
      useTranscriptSidecarStore.getState().getSummaryState('session:draft-rec-1').streamingContent
    ).toBe('Draft partial stream');
    expect(
      useTranscriptSidecarStore.getState().getSummaryState('history-other').record?.content
    ).toBe('Existing other summary');

    invokeGate.resolve({
      taskId: 'summary-task-id',
      taskType: 'summary',
      jobHistoryId: null,
      summary: {
        activeTemplateId: 'general',
        record: {
          templateId: 'general',
          content: 'Final draft summary',
          generatedAt: '2026-05-05T00:00:00.000Z',
          sourceFingerprint: 'draft-fp',
        },
      },
    });
    await jobPromise;

    expect(
      useTranscriptSidecarStore.getState().getSummaryState('session:draft-rec-1').record?.content
    ).toBe('Final draft summary');
    expect(
      useTranscriptSidecarStore.getState().getSummaryState('history-other').record?.content
    ).toBe('Existing other summary');
    expect(historyService.saveSummary).not.toHaveBeenCalled();
  });

  it('runs Summary concurrently while Channel 3 lock is held and deduplicates same-document requests', async () => {
    const { documentMutationCoordinator } = await import('../task/documentMutationCoordinator');

    useTranscriptStore.setState({
      config: createSummaryReadyConfig(),
      activeSessionId: 'history-locked',
      sessions: {
        default: { ...useTranscriptStore.getState().sessions.default },
        'history-locked': {
          ...useTranscriptStore.getState().sessions.default,
          sourceHistoryId: 'history-locked',
          segments: [{ id: 'l-1', text: 'Locked transcript', start: 0, end: 2, isFinal: true }],
        },
      },
    });

    const lockGate = Promise.withResolvers<void>();
    const summaryGate = Promise.withResolvers<any>();
    vi.mocked(invoke).mockImplementation(async () => summaryGate.promise);

    // Acquire Channel 3 exclusive document lock on history-locked
    const heldLock = documentMutationCoordinator.runExclusive(
      { sessionId: 'history-locked', historyId: 'history-locked' },
      { taskType: 'polish' },
      async () => {
        // Trigger duplicate concurrent Summary calls while lock is STILL held
        const p1 = summaryService.generateSummary({
          templateId: 'general',
          sessionId: 'history-locked',
        });
        const p2 = summaryService.generateSummary({
          templateId: 'general',
          sessionId: 'history-locked',
        });

        await vi.waitFor(() => {
          expect(invoke).toHaveBeenCalledTimes(1);
        });

        summaryGate.resolve({
          taskId: 'summary-task-id',
          taskType: 'summary',
          jobHistoryId: 'history-locked',
          summary: {
            activeTemplateId: 'general',
            record: {
              templateId: 'general',
              content: 'Concurrent summary under lock',
              generatedAt: '2026-05-05T00:00:00.000Z',
              sourceFingerprint: 'lock-fp',
            },
          },
        });
        await Promise.all([p1, p2]);
        await lockGate.promise;
      }
    );

    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledTimes(1);
    });
    lockGate.resolve();
    await heldLock;
  });
});
