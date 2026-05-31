import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TranslateButton } from '../TranslateButton';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { useConfigStore } from '../../stores/configStore';

// Mock dependencies
vi.mock('react-i18next', async (importOriginal) => {
    const actual = await importOriginal<typeof import('react-i18next')>();
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
    useDialogStore: (selector: any) => selector({
        showError: vi.fn(),
    }),
}));

describe('TranslateButton', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        localStorage.clear();

        // Reset store
        useTranscriptStore.getState().setSegments([
            { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true },
        ]);
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
            } as any
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

        expect(screen.getByText('translation.start')).toBeDefined();
        expect(screen.getByText('translation.show_bilingual')).toBeDefined();
        expect(screen.getByPlaceholderText('translation.search_placeholder')).toBeDefined();
        expect(screen.getByText('translation.commonly_used')).toBeDefined();
        expect(screen.getByText('translation.all_languages')).toBeDefined();
    });

    it('filters languages by search query', () => {
        render(<TranslateButton />);
        fireEvent.click(screen.getByRole('button', { expanded: false }));

        const searchInput = screen.getByPlaceholderText('translation.search_placeholder');

        // Let's filter to Spanish
        fireEvent.change(searchInput, { target: { value: 'spanish' } });

        expect(screen.getByText('translation.search_results')).toBeDefined();
        expect(screen.queryByText('translation.commonly_used')).toBeNull();
        expect(screen.getByText('Spanish')).toBeDefined();
    });

    it('displays no results when search query has no match', () => {
        render(<TranslateButton />);
        fireEvent.click(screen.getByRole('button', { expanded: false }));

        const searchInput = screen.getByPlaceholderText('translation.search_placeholder');
        fireEvent.change(searchInput, { target: { value: 'nonexistentlanguagequery' } });

        expect(screen.getByText('translation.no_results')).toBeDefined();
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
});
