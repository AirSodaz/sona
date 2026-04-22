import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptSummaryPanel } from '../TranscriptSummaryPanel';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { DEFAULT_CONFIG } from '../../stores/configStore';
import { addLlmModel, createLlmSettings, setFeatureModelSelection, updateProviderSetting } from '../../services/llmConfig';

const mockLoadSummary = vi.fn();
const mockSetActiveTemplate = vi.fn();
const mockGenerateSummary = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'summary.empty_state' && options?.template) {
        return `summary.empty_state:${options.template}`;
      }
      if (key === 'summary.generating_progress' && options?.progress !== undefined) {
        return `summary.generating_progress:${options.progress}`;
      }
      if (key === 'summary.generating_short_progress' && options?.progress !== undefined) {
        return `summary.generating_short_progress:${options.progress}`;
      }
      return key;
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../services/summaryService', async () => {
  const actual = await vi.importActual<typeof import('../../services/summaryService')>('../../services/summaryService');
  return {
    ...actual,
    summaryService: {
      loadSummary: (...args: unknown[]) => mockLoadSummary(...args),
      setActiveTemplate: (...args: unknown[]) => mockSetActiveTemplate(...args),
      generateSummary: (...args: unknown[]) => mockGenerateSummary(...args),
    },
  };
});

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

function createSummaryConfigWithoutModel() {
  const readyConfig = createSummaryReadyConfig();

  return {
    ...readyConfig,
    llmSettings: setFeatureModelSelection(readyConfig.llmSettings, 'summary', undefined),
  };
}

function createSummaryConfigWithoutApiKey() {
  const readyConfig = createSummaryReadyConfig();

  return {
    ...readyConfig,
    llmSettings: updateProviderSetting(readyConfig.llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: '',
    }),
  };
}

describe('TranscriptSummaryPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
      configurable: true,
    });

    useTranscriptStore.setState({
      segments: [],
      sourceHistoryId: null,
      summaryStates: {},
      config: createSummaryReadyConfig(),
    });
  });

  it('starts collapsed and shows lightweight stale status without rendering content', () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Updated transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        current: {
          activeTemplate: 'general',
          records: {
            general: {
              template: 'general',
              content: 'Saved summary content',
              generatedAt: '2026-04-22T10:00:00.000Z',
              sourceFingerprint: 'older-fingerprint',
            },
          },
          isGenerating: false,
          generationProgress: 0,
        },
      },
    });

    render(<TranscriptSummaryPanel />);

    expect(screen.getByText('summary.title')).toBeDefined();
    expect(screen.getByRole('button', { name: 'summary.expand' }).getAttribute('aria-expanded')).toBe('false');
    expect(document.querySelector('.transcript-summary-panel-gutter-label')).toBeNull();
    expect(document.querySelector('.transcript-summary-panel-toggle > .transcript-summary-panel-chevron')).not.toBeNull();
    expect(screen.getByText('summary.stale_short')).toBeDefined();
    expect(document.querySelector('.transcript-summary-panel-body')).toBeNull();
    expect(screen.queryByText('summary.templates.general')).toBeNull();
    expect(screen.queryByText('Saved summary content')).toBeNull();
    expect(screen.queryByText('summary.stale')).toBeNull();
  });

  it('expands on demand and supports template switching, generating, copying, and collapsing again', async () => {
    useTranscriptStore.setState({
      sourceHistoryId: 'history-1',
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        'history-1': {
          activeTemplate: 'general',
          records: {
            general: {
              template: 'general',
              content: 'History summary',
              generatedAt: '2026-04-22T10:00:00.000Z',
              sourceFingerprint: '1:Transcript text:0:1:true',
            },
          },
          isGenerating: false,
          generationProgress: 0,
        },
      },
    });

    render(<TranscriptSummaryPanel />);

    await waitFor(() => {
      expect(mockLoadSummary).toHaveBeenCalledWith('history-1');
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'summary.expand' }));
    });

    const panelBody = document.querySelector('.transcript-summary-panel-body') as HTMLDivElement;

    expect(screen.getByRole('button', { name: 'summary.collapse' }).getAttribute('aria-expanded')).toBe('true');
    expect(panelBody).toBeDefined();
    expect(screen.getByText('History summary')).toBeDefined();

    await act(async () => {
      fireEvent.click(screen.getByText('summary.templates.meeting'));
    });
    expect(mockSetActiveTemplate).toHaveBeenCalledWith('meeting');

    await act(async () => {
      fireEvent.click(screen.getByText('summary.regenerate'));
    });
    expect(mockGenerateSummary).toHaveBeenCalledWith('general');

    await act(async () => {
      fireEvent.click(screen.getByText('summary.copy'));
    });

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('History summary');
    await waitFor(() => {
      expect(screen.getByText('summary.copied')).toBeDefined();
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'summary.collapse' }));
    });

    expect(screen.getByRole('button', { name: 'summary.expand' }).getAttribute('aria-expanded')).toBe('false');
    expect(document.querySelector('.transcript-summary-panel-body')).toBeNull();
    expect(screen.queryByText('History summary')).toBeNull();
  });

  it('shows generating status in both collapsed and expanded states while disabling actions', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        current: {
          activeTemplate: 'general',
          records: {},
          isGenerating: true,
          generationProgress: 42,
        },
      },
    });

    render(<TranscriptSummaryPanel />);

    expect(screen.getByText('summary.generating_short_progress:42')).toBeDefined();
    expect(document.querySelector('.transcript-summary-panel-body')).toBeNull();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'summary.expand' }));
    });

    const panelBody = document.querySelector('.transcript-summary-panel-body') as HTMLDivElement;

    expect(panelBody).toBeDefined();
    expect(screen.queryByText('summary.generating_short_progress:42')).toBeNull();
    expect(screen.getAllByText('summary.generating_progress:42').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole('button', { name: 'summary.generating_progress:42' }).hasAttribute('disabled')).toBe(true);
    expect((screen.getByText('summary.templates.general') as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'summary.copy' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('does not render when AI Summary is disabled in config', () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      config: {
        ...createSummaryReadyConfig(),
        summaryEnabled: false,
      },
    });

    render(<TranscriptSummaryPanel />);

    expect(screen.queryByText('summary.title')).toBeNull();
    expect(mockLoadSummary).not.toHaveBeenCalled();
  });

  it('does not render when the summary model is not assigned', () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      config: createSummaryConfigWithoutModel(),
    });

    render(<TranscriptSummaryPanel />);

    expect(screen.queryByText('summary.title')).toBeNull();
    expect(mockLoadSummary).not.toHaveBeenCalled();
  });

  it('does not render when the assigned summary model is missing required credentials', () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      config: createSummaryConfigWithoutApiKey(),
    });

    render(<TranscriptSummaryPanel />);

    expect(screen.queryByText('summary.title')).toBeNull();
    expect(mockLoadSummary).not.toHaveBeenCalled();
  });
});
