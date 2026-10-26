import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { translationService } from '../../services/translationService';
import { useConfigStore } from '../../stores/configStore';
import { useProjectStore } from '../../stores/projectStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { DEFAULT_PROJECT_PIPELINE } from '../../types/project';
import { TranslateButton } from '../TranslateButton';

// Mock dependencies
vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => key,
      i18n: { language: 'en' },
    }),
  };
});

// Mock useDialogStore
vi.mock('../../stores/dialogStore', () => ({
  useDialogStore: (selector: any) =>
    selector({
      showError: vi.fn(),
    }),
}));
vi.mock('../../services/translationService', () => ({
  translationService: {
    translateCurrentTranscript: vi.fn(),
  },
}));

const mockToggleLiveTranslation = vi.fn().mockResolvedValue(undefined);
vi.mock('../../services/tauri/recognizer', () => ({
  toggleLiveTranslation: (...args: unknown[]) => mockToggleLiveTranslation(...args),
}));

const mockIsModelInstalled = vi.fn().mockResolvedValue(true);
vi.mock('../../services/modelService', () => ({
  modelService: {
    isModelInstalled: (...args: unknown[]) => mockIsModelInstalled(...args),
  },
}));

describe('TranslateButton', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();

    useProjectStore.setState({ activeProjectId: null, projects: [] });
    // Reset store
    useTranscriptStore
      .getState()
      .setSegments([{ id: '1', start: 0, end: 1, text: 'Hello', isFinal: true }]);
    useTranscriptStore.setState({ llmStates: {} });

    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        translationLanguage: 'zh',
        llmSettings: {
          activeProvider: 'open_ai',
          providers: {
            open_ai: {
              apiHost: 'https://api.test',
              apiKey: 'test-key',
              temperature: 0.7,
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
            translationModelId: 'open-ai-test',
          },
        },
      } as any,
    });

    useTranscriptStore.setState({
      config: useConfigStore.getState().config as any,
    });
  });

  it('renders the translate button', () => {
    render(<TranslateButton />);
    const button = screen.getByRole('button', { expanded: false });
    expect(button).toBeDefined();
  });

  it('shows dropdown options when clicked', () => {
    render(<TranslateButton />);
    const button = screen.getByRole('button', { expanded: false });
    fireEvent.click(button);

    screen.getByText('translation.start');
    screen.getByText('translation.hide_bilingual');
    screen.getByPlaceholderText('translation.search_placeholder');
    screen.getByText('translation.commonly_used');
    screen.getByText('translation.all_languages');
  });

  it('toggles bilingual translation visibility when toggle button is clicked', () => {
    render(<TranslateButton />);
    const button = screen.getByRole('button', { expanded: false });
    fireEvent.click(button);

    const toggleBtn = screen.getByText('translation.hide_bilingual');
    fireEvent.click(toggleBtn);

    expect(useTranscriptStore.getState().getLlmState('current').isTranslationVisible).toBe(false);

    // Reopen menu
    fireEvent.click(button);
    const showBtn = screen.getByText('translation.show_bilingual');
    fireEvent.click(showBtn);

    expect(useTranscriptStore.getState().getLlmState('current').isTranslationVisible).toBe(true);
  });

  it('filters languages by search query', () => {
    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    const searchInput = screen.getByPlaceholderText('translation.search_placeholder');

    // Let's filter to Spanish
    fireEvent.change(searchInput, { target: { value: 'spanish' } });

    screen.getByText('translation.search_results');
    expect(screen.queryByText('translation.commonly_used')).toBeNull();
    screen.getByText('Spanish');
  });

  it('displays no results when search query has no match', () => {
    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    const searchInput = screen.getByPlaceholderText('translation.search_placeholder');
    fireEvent.change(searchInput, { target: { value: 'nonexistentlanguagequery' } });

    screen.getByText('translation.no_results');
  });

  it('persists selected language to recent languages on selection', async () => {
    const { unmount } = render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    // Find Afrikaans in all languages list and click it
    const afrikaansBtn = screen.getByText('Afrikaans');
    fireEvent.click(afrikaansBtn);

    // Wait for state updates
    expect(useConfigStore.getState().config.translationLanguage).toBe('af');

    // Unmount first instance to prevent duplicate elements in JSDOM
    unmount();

    // Re-open menu to verify it is stored in recent languages
    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    const storedRecents = localStorage.getItem('sona_recent_translation_languages');
    expect(storedRecents).toContain('af');
  });

  it('updates project pipeline target language when in project with pipeline enabled', async () => {
    const mockUpdateProjectPipeline = vi.fn().mockResolvedValue(undefined);
    useProjectStore.setState({
      activeProjectId: 'proj-1',
      projects: [
        {
          id: 'proj-1',
          name: 'Project 1',
          createdAt: 1000,
          updatedAt: 1000,
          pipeline: {
            ...DEFAULT_PROJECT_PIPELINE,
            enabled: true,
            autoPolish: false,
            autoTranslate: true,
            targetLanguage: 'zh',
          },
        },
      ],
      updateProjectPipeline: mockUpdateProjectPipeline,
    });

    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    const afrikaansBtn = screen.getByText('Afrikaans');
    fireEvent.click(afrikaansBtn);

    await waitFor(() => {
      expect(mockUpdateProjectPipeline).toHaveBeenCalledWith(
        'proj-1',
        expect.objectContaining({
          targetLanguage: 'af',
        })
      );
      expect(useConfigStore.getState().config.translationLanguage).toBe('af');
    });
  });

  it('passes effectiveTargetLang to translateCurrentTranscript when translation starts', async () => {
    useProjectStore.setState({
      activeProjectId: 'proj-1',
      projects: [
        {
          id: 'proj-1',
          name: 'Project 1',
          createdAt: 1000,
          updatedAt: 1000,
          pipeline: {
            ...DEFAULT_PROJECT_PIPELINE,
            enabled: true,
            autoPolish: false,
            autoTranslate: true,
            targetLanguage: 'es',
          },
        },
      ],
    });

    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    const startBtn = screen.getByText('translation.start');
    fireEvent.click(startBtn);

    expect(translationService.translateCurrentTranscript).toHaveBeenCalledWith('es');
  });

  it('renders A => B language selector and swaps languages when clicking swap button', () => {
    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    const swapBtn = screen.getByRole('button', { name: 'translation.swap_languages' });
    expect(swapBtn).toBeDefined();

    fireEvent.click(swapBtn);
    const config = useConfigStore.getState().config;
    expect(config.realtimeTranslationSourceLanguage).toBe('zh');
  });

  it('renders realtime translation toggle card during active recording and calls toggleLiveTranslation', async () => {
    useTranscriptRuntimeStore.setState({ isRecording: true });
    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    screen.getByText('translation.realtime_toggle');
    screen.getByText('translation.realtime_toggle_desc');
    expect(screen.queryByText('translation.start')).toBeNull();

    const toggleSwitch = screen.getByRole('switch');
    fireEvent.click(toggleSwitch);

    await waitFor(() => {
      expect(mockToggleLiveTranslation).toHaveBeenCalledWith(
        expect.objectContaining({
          enabled: true,
        })
      );
    });

    useTranscriptRuntimeStore.setState({ isRecording: false });
  });

  it('displays download badge for uninstalled local ONNX models', async () => {
    mockIsModelInstalled.mockImplementation(async (id: unknown) => id === 'opus-mt-zh-en');
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        realtimeTranslationEngine: 'onnx',
        realtimeTranslationSourceLanguage: 'zh',
        translationLanguage: 'ja',
        llmSettings: {
          activeProvider: 'local',
          providers: {},
          models: {
            local_nmt: {
              id: 'local_nmt',
              provider: 'local',
              model: 'opus-mt-zh-en',
            },
          },
          modelOrder: ['local_nmt'],
          selections: {
            translationModelId: 'local_nmt',
          },
        },
      },
    });
    useTranscriptStore.setState({
      config: useConfigStore.getState().config,
    });

    render(<TranslateButton />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    await waitFor(() => {
      screen.getByText('models.not_downloaded');
    });
  });
});
