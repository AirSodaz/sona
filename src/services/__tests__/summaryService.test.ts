import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { addLlmModel, createLlmSettings, setFeatureModelSelection, updateProviderSetting } from '../llmConfig';
import { summaryService } from '../summaryService';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { DEFAULT_CONFIG } from '../../stores/configStore';

const mockCreateLlmTaskId = vi.fn();
const mockListenToLlmTaskProgress = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../llmTaskService', async () => {
  const actual = await vi.importActual<typeof import('../llmTaskService')>('../llmTaskService');
  return {
    ...actual,
    createLlmTaskId: (...args: unknown[]) => mockCreateLlmTaskId(...args),
    listenToLlmTaskProgress: (...args: unknown[]) => mockListenToLlmTaskProgress(...args),
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

  return {
    ...DEFAULT_CONFIG,
    llmSettings,
  };
}

describe('summaryService', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    mockCreateLlmTaskId.mockReturnValue('summary-task-id');
    mockListenToLlmTaskProgress.mockResolvedValue(vi.fn());

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
    vi.mocked(invoke).mockResolvedValue({
      template: 'meeting',
      content: 'Meeting summary',
    });

    await summaryService.generateSummary('meeting');

    expect(invoke).toHaveBeenCalledWith('summarize_transcript', {
      request: {
        taskId: 'summary-task-id',
        template: 'meeting',
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
        activeTemplate: 'meeting',
        records: expect.objectContaining({
          meeting: expect.objectContaining({
            content: 'Meeting summary',
          }),
        }),
      }),
    );
    expect(useTranscriptStore.getState().getSummaryState('history-a').records.meeting?.content).toBe('Meeting summary');
    expect(useTranscriptStore.getState().getSummaryState('history-a').isGenerating).toBe(false);
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
        template: 'general',
        content: 'Current summary',
      };
    });

    await summaryService.generateSummary('general');

    expect(historyService.saveSummary).toHaveBeenCalledWith(
      'history-new',
      expect.objectContaining({
        records: expect.objectContaining({
          general: expect.objectContaining({
            content: 'Current summary',
          }),
        }),
      }),
    );
    expect(useTranscriptStore.getState().summaryStates.current).toBeUndefined();
    expect(useTranscriptStore.getState().getSummaryState('history-new').records.general?.content).toBe('Current summary');
  });

  it('hydrates summary sidecars into the store', async () => {
    const { historyService } = await import('../historyService');
    vi.mocked(historyService.loadSummary).mockResolvedValue({
      activeTemplate: 'lecture',
      records: {
        lecture: {
          template: 'lecture',
          content: 'Lecture summary',
          generatedAt: '2026-04-22T10:00:00.000Z',
          sourceFingerprint: 'fingerprint-a',
        },
      },
    });

    await summaryService.loadSummary('history-lecture');

    expect(useTranscriptStore.getState().getSummaryState('history-lecture')).toEqual(expect.objectContaining({
      activeTemplate: 'lecture',
      records: expect.objectContaining({
        lecture: expect.objectContaining({
          content: 'Lecture summary',
        }),
      }),
    }));
  });
});
