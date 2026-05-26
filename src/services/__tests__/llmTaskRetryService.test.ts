import { beforeEach, describe, expect, it, vi } from 'vitest';
import { retryLlmTaskFromLedger } from '../llmTaskRetryService';
import { historyService } from '../historyService';
import { polishService } from '../polishService';
import { summaryService } from '../summaryService';
import { translationService } from '../translationService';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { patchTaskLedgerRecord } from '../taskLedgerBuilders';
import type { TaskLedgerRecord } from '../../types/taskLedger';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import { useConfigStore } from '../../stores/configStore';
import { useEffectiveConfigStore } from '../../stores/effectiveConfigStore';

vi.mock('../historyService', () => ({
  historyService: {
    loadTranscript: vi.fn(),
  },
}));

vi.mock('../polishService', () => ({
  polishService: {
    retryPolishTranscriptJob: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../summaryService', () => ({
  summaryService: {
    retrySummaryTranscriptJob: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../translationService', () => ({
  translationService: {
    retryTranslateTranscriptJob: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../taskLedgerBuilders', () => ({
  patchTaskLedgerRecord: vi.fn(),
}));

function makeTask(overrides: Partial<TaskLedgerRecord> = {}): TaskLedgerRecord {
  return {
    id: 'llm-old-task',
    kind: 'llmTranslate',
    status: 'failed',
    title: 'Translate',
    progress: 0,
    createdAt: 100,
    updatedAt: 200,
    retryable: true,
    cancelable: false,
    recoverable: false,
    historyId: 'history-a',
    targetLanguage: 'ja',
    ...overrides,
  };
}

const segments = [
  { id: '1', text: 'hello', start: 0, end: 1, isFinal: true },
];

function createLlmReadyConfig() {
  return buildTestConfig({
    summaryEnabled: true,
    translationLanguage: 'zh',
    llmSettings: {
      activeProvider: 'open_ai',
      providers: {
        open_ai: {
          apiHost: 'https://api.openai.com',
          apiKey: 'test-key',
        },
      },
      models: {
        'open-ai-test': {
          id: 'open-ai-test',
          provider: 'open_ai',
          model: 'gpt-4o-mini',
        },
      },
      modelOrder: ['open-ai-test'],
      selections: {
        polishModelId: 'open-ai-test',
        summaryModelId: 'open-ai-test',
        translationModelId: 'open-ai-test',
      },
    },
  });
}

function setConfig(config = createLlmReadyConfig()) {
  useConfigStore.setState((state) => ({
    ...state,
    config,
  }));
  useEffectiveConfigStore.setState((state) => ({
    ...state,
    config,
  }));
  void useEffectiveConfigStore.getState().syncConfig();
}

describe('retryLlmTaskFromLedger', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTranscriptStores();
    useTranscriptSessionStore.setState({
      segments: [],
      sourceHistoryId: null,
    });
    setConfig();
  });

  it('uses in-memory segments when retrying the currently open history record', async () => {
    useTranscriptSessionStore.setState({
      segments,
      sourceHistoryId: 'history-a',
    });

    await retryLlmTaskFromLedger(makeTask());

    expect(historyService.loadTranscript).not.toHaveBeenCalled();
    expect(translationService.retryTranslateTranscriptJob).toHaveBeenCalledWith({
      segments,
      historyId: 'history-a',
      targetLanguage: 'ja',
    });
  });

  it('loads saved transcript segments when retrying a background history record', async () => {
    vi.mocked(historyService.loadTranscript).mockResolvedValue(segments);

    await retryLlmTaskFromLedger(makeTask({ historyId: 'history-b' }));

    expect(historyService.loadTranscript).toHaveBeenCalledWith('history-b.json');
    expect(translationService.retryTranslateTranscriptJob).toHaveBeenCalledWith({
      segments,
      historyId: 'history-b',
      targetLanguage: 'ja',
    });
  });

  it('allows unsaved current retries only when the current unsaved session still has segments', async () => {
    useTranscriptSessionStore.setState({
      segments,
      sourceHistoryId: null,
    });

    await retryLlmTaskFromLedger(makeTask({
      kind: 'llmPolish',
      historyId: undefined,
      targetLanguage: undefined,
    }));

    expect(polishService.retryPolishTranscriptJob).toHaveBeenCalledWith({
      segments,
      historyId: null,
    });
  });

  it('routes summary retries with the ledger template id', async () => {
    useTranscriptSessionStore.setState({
      segments,
      sourceHistoryId: 'history-a',
    });

    await retryLlmTaskFromLedger(makeTask({
      kind: 'llmSummary',
      templateId: 'meeting',
      targetLanguage: undefined,
    }));

    expect(summaryService.retrySummaryTranscriptJob).toHaveBeenCalledWith({
      segments,
      historyId: 'history-a',
      templateId: 'meeting',
    });
  });

  it('keeps the old ledger task and records a preflight error when retry cannot start', async () => {
    await expect(retryLlmTaskFromLedger(makeTask({
      historyId: undefined,
      targetLanguage: undefined,
    }))).rejects.toThrow('Transcript is no longer available for retry.');

    expect(patchTaskLedgerRecord).toHaveBeenCalledWith('llm-old-task', expect.objectContaining({
      status: 'failed',
      retryable: true,
      cancelable: false,
      errorMessage: 'Transcript is no longer available for retry.',
    }));
    expect(translationService.retryTranslateTranscriptJob).not.toHaveBeenCalled();
  });

  it('records a configuration preflight error without storing request text', async () => {
    useTranscriptSessionStore.setState({
      segments,
      sourceHistoryId: null,
    });
    const config = buildTestConfig({
      llmSettings: undefined,
    });

    await expect(retryLlmTaskFromLedger(makeTask({
      historyId: undefined,
      targetLanguage: undefined,
    }), { config })).rejects.toThrow('LLM Service not fully configured.');

    expect(patchTaskLedgerRecord).toHaveBeenCalledWith('llm-old-task', expect.objectContaining({
      errorMessage: 'LLM Service not fully configured.',
    }));
  });
});
