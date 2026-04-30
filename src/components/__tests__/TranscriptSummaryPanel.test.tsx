import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptSummaryPanel } from '../TranscriptSummaryPanel';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { DEFAULT_CONFIG } from '../../stores/configStore';
import { addLlmModel, createLlmSettings, setFeatureModelSelection, updateProviderSetting } from '../../services/llm/state';

const mockLoadSummary = vi.fn();
const mockSetActiveTemplate = vi.fn();
const mockGenerateSummary = vi.fn();
const mockUpdateSummaryRecord = vi.fn();
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
      updateSummaryRecord: (...args: unknown[]) => mockUpdateSummaryRecord(...args),
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
    mockLoadSummary.mockResolvedValue(undefined);
    mockSetActiveTemplate.mockResolvedValue(undefined);
    mockGenerateSummary.mockResolvedValue(undefined);
    mockUpdateSummaryRecord.mockResolvedValue(undefined);
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
          activeTemplateId: 'general',
          record: {
            templateId: 'general',
            content: 'History summary',
            generatedAt: '2026-04-22T10:00:00.000Z',
            sourceFingerprint: '1:Transcript text:0:1:true',
          },
          streamingContent: undefined,
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
    expect(screen.getByDisplayValue('History summary')).toBeDefined();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'summary.templates.general' }));
    });
    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'summary.templates.meeting' })).toBeDefined();
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('option', { name: 'summary.templates.meeting' }));
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

  it('auto-saves draft edits on blur, before template switches, before regenerate, and before close', async () => {
    useTranscriptStore.setState({
      sourceHistoryId: 'history-1',
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        'history-1': {
          activeTemplateId: 'general',
          record: {
            templateId: 'general',
            content: 'History summary',
            generatedAt: '2026-04-22T10:00:00.000Z',
            sourceFingerprint: '1:Transcript text:0:1:true',
          },
          streamingContent: undefined,
          isGenerating: false,
          generationProgress: 0,
        },
      },
    });

    render(<TranscriptSummaryPanel isOpen={true} onClose={mockOnClose} />);

    const textarea = screen.getByRole('textbox');

    await act(async () => {
      fireEvent.change(textarea, { target: { value: 'Blur draft' } });
      fireEvent.blur(textarea);
    });
    await waitFor(() => {
      expect(mockUpdateSummaryRecord).toHaveBeenCalledWith('Blur draft');
    });

    await act(async () => {
      fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Template draft' } });
      fireEvent.click(screen.getByRole('button', { name: 'summary.templates.general' }));
    });
    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'summary.templates.meeting' })).toBeDefined();
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('option', { name: 'summary.templates.meeting' }));
    });
    await waitFor(() => {
      expect(mockUpdateSummaryRecord).toHaveBeenCalledWith('Template draft');
      expect(mockSetActiveTemplate).toHaveBeenCalledWith('meeting');
    });

    await act(async () => {
      fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Generate draft' } });
      fireEvent.click(screen.getByText('summary.regenerate'));
    });
    await waitFor(() => {
      expect(mockUpdateSummaryRecord).toHaveBeenCalledWith('Generate draft');
      expect(mockGenerateSummary).toHaveBeenCalledWith('general');
    });

    await act(async () => {
      fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Close draft' } });
      fireEvent.click(screen.getByLabelText('common.close'));
    });
    await waitFor(() => {
      expect(mockUpdateSummaryRecord).toHaveBeenCalledWith('Close draft');
      expect(mockOnClose).toHaveBeenCalled();
    });
  });

  it('shows generating status and progress', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        current: {
          activeTemplateId: 'general',
          record: undefined,
          streamingContent: 'Streaming summary text',
          isGenerating: true,
          generationProgress: 42,
        },
      },
    });

    await act(async () => {
      render(<TranscriptSummaryPanel isOpen={true} onClose={mockOnClose} />);
      await Promise.resolve();
    });

    expect(screen.getByText('summary.generating_progress:42')).toBeDefined();
    expect(screen.getByText('summary.generating_short')).toBeDefined();
    expect((screen.getByRole('button', { name: 'summary.generating_short' }) as HTMLButtonElement).disabled).toBe(true);
    await waitFor(() => {
      expect(screen.getByDisplayValue('Streaming summary text')).toBeDefined();
    });
  });

  it('shows a textarea immediately when no record exists', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        current: {
          activeTemplateId: 'general',
          record: undefined,
          streamingContent: undefined,
          isGenerating: false,
          generationProgress: 0,
        },
      },
    });

    render(<TranscriptSummaryPanel isOpen={true} onClose={mockOnClose} />);

    expect(screen.getByRole('textbox')).toBeDefined();
    expect(screen.queryByRole('button', { name: 'summary.start_writing' })).toBeNull();
  });

  it('keeps unsaved streamed content visible after generation stops', async () => {
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      summaryStates: {
        current: {
          activeTemplateId: 'general',
          record: undefined,
          streamingContent: 'Recoverable streamed summary',
          isGenerating: false,
          generationProgress: 0,
        },
      },
    });

    await act(async () => {
      render(<TranscriptSummaryPanel isOpen={true} onClose={mockOnClose} />);
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(screen.getByDisplayValue('Recoverable streamed summary')).toBeDefined();
    });
  });

  it('keeps the panel open for manual editing when summary generation is unavailable', () => {
    const readyConfig = createSummaryReadyConfig();
    useTranscriptStore.setState({
      segments: [
        { id: '1', text: 'Transcript text', start: 0, end: 1, isFinal: true },
      ],
      config: {
        ...readyConfig,
        llmSettings: updateProviderSetting(readyConfig.llmSettings, 'open_ai', {
          apiHost: 'https://api.openai.com',
          apiKey: '',
        }),
      },
    });

    render(<TranscriptSummaryPanel isOpen={true} onClose={mockOnClose} />);

    expect(screen.getByText('summary.manual_only_hint')).toBeDefined();
    expect((screen.getByRole('button', { name: 'summary.generate' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('textbox')).toBeDefined();
  });
});
