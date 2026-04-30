import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { addLlmModel, createLlmSettings, setFeatureModelSelection, updateProviderSetting } from '../llm/state';
import { summaryService } from '../summaryService';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { buildTestConfig } from '../../test-utils/configTestUtils';

const mockCreateLlmTaskId = vi.fn();
const mockListenToLlmTaskProgress = vi.fn();
const mockListenToLlmTaskText = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../llmTaskService', async () => {
  const actual = await vi.importActual<typeof import('../llmTaskService')>('../llmTaskService');
  return {
    ...actual,
    createLlmTaskId: (...args: unknown[]) => mockCreateLlmTaskId(...args),
    listenToLlmTaskProgress: (...args: unknown[]) => mockListenToLlmTaskProgress(...args),
    listenToLlmTaskText: (...args: unknown[]) => mockListenToLlmTaskText(...args),
  };
});

vi.mock('../historyService', () => ({
  historyService: {
    loadSummary: vi.fn(),
    saveSummary: vi.fn(),
    deleteSummary: vi.fn(),
  },
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

    useTranscriptStore.setState({
      segments: [],
      sourceHistoryId: null,
      summaryStates: {},
      config: createSummaryReadyConfig(),
    });
  });

  it('builds the summarize request and persists history-backed summaries', async () => {
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
      templateId: 'meeting',
      content: 'Meeting summary',
    });

    await summaryService.generateSummary('meeting');

    expect(invoke).toHaveBeenCalledWith('summarize_transcript', {
      request: {
        taskId: 'summary-task-id',
        template: {
          id: 'meeting',
          name: 'Meeting',
          instructions: expect.stringContaining('Meeting overview.'),
          builtIn: true,
        },
        config: expect.objectContaining({
          apiKey: 'test-key',
          model: 'gpt-4o-mini',
          temperature: 0.7,
        }),
        segments: [
          { id: '1', text: 'Hello world', start: 0, end: 2, isFinal: true },
          { id: '2', text: 'Next point', start: 2, end: 4, isFinal: true },
        ],
      },
    });
    expect(historyService.saveSummary).toHaveBeenCalledWith(
      'history-a',
      expect.objectContaining({
        activeTemplateId: 'meeting',
        record: expect.objectContaining({
          content: 'Meeting summary',
        }),
      }),
    );
    expect(useTranscriptStore.getState().getSummaryState('history-a').record?.content).toBe('Meeting summary');
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
        templateId: 'general',
        content: 'Final summary',
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
        templateId: 'general',
        content: 'Current summary',
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

    await expect(summaryService.generateSummary('general')).rejects.toThrow('Summary is disabled.');
    expect(invoke).not.toHaveBeenCalled();
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

    await expect(summaryService.generateSummary('general')).rejects.toThrow('LLM Service not fully configured.');
    expect(invoke).not.toHaveBeenCalled();
  });
});
