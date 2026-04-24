import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptSummaryPanel } from '../TranscriptSummaryPanel';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { DEFAULT_CONFIG } from '../../stores/configStore';
import { addLlmModel, createLlmSettings, setFeatureModelSelection, updateProviderSetting } from '../../services/llmConfig';

const mockLoadSummary = vi.fn();
const mockSetActiveTemplate = vi.fn();
const mockGenerateSummary = vi.fn();
const mockOnClose = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'summary.empty_state' && options?.template) {
        return `summary.empty_state:${options.template}`;
      }
      if (key === 'summary.generating_progress' && options?.progress !== undefined) {
        return `summary.generating_progress:${options.progress}`;
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

  it('renders nothing when isOpen is false', () => {
    render(<TranscriptSummaryPanel isOpen={false} onClose={mockOnClose} />);
    expect(screen.queryByText('summary.title')).toBeNull();
  });

  it('renders content when isOpen is true and supports template switching, generating, and copying', async () => {
    useTranscriptStore.setState({
      sourceHistoryId: 'history-1',
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        'history-1': {
          activeTemplate: 'general',
          record: {
            template: 'general',
            content: 'History summary',
            generatedAt: '2026-04-22T10:00:00.000Z',
            sourceFingerprint: '1:Transcript text:0:1:true',
          },
          isGenerating: false,
          generationProgress: 0,
        },
      },
    });

    render(<TranscriptSummaryPanel isOpen={true} onClose={mockOnClose} />);

    await waitFor(() => {
      expect(mockLoadSummary).toHaveBeenCalledWith('history-1');
    });

    expect(screen.getByText('summary.title')).toBeDefined();
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
      fireEvent.click(screen.getByLabelText('common.close'));
    });
    expect(mockOnClose).toHaveBeenCalled();
  });

  it('shows generating status and progress', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        current: {
          activeTemplate: 'general',
          record: undefined,
          isGenerating: true,
          generationProgress: 42,
        },
      },
    });

    render(<TranscriptSummaryPanel isOpen={true} onClose={mockOnClose} />);

    expect(screen.getByText('summary.generating_progress:42')).toBeDefined();
    expect(screen.getByText('summary.generating_short')).toBeDefined();
    expect((screen.getByRole('button', { name: 'summary.generating_short' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByText('summary.templates.general') as HTMLButtonElement).disabled).toBe(true);
  });
});
