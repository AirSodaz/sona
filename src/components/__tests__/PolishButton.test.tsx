import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { PolishButton } from '../PolishButton';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useConfigStore } from '../../stores/configStore';
import { polishService } from '../../services/polishService';

// Mock dependencies
vi.mock('react-i18next', async (importOriginal) => {
    const actual = await importOriginal<typeof import('react-i18next')>();
    return {
        ...actual,
        useTranslation: () => ({
            t: (key: string) => key,
        }),
    };
});

vi.mock('../../services/polishService', () => ({
    polishService: {
        polishTranscript: vi.fn(),
    },
}));

// Mock useDialogStore
vi.mock('../../stores/dialogStore', () => ({
    useDialogStore: (selector: any) => selector({
        showError: vi.fn(),
    }),
}));

describe('PolishButton', () => {
    beforeEach(() => {
        vi.clearAllMocks();

        // Reset store
        useTranscriptStore.setState({
            segments: [
                { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true },
            ],
            llmStates: {},
        });

        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
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
                        polishModelId: 'open-ai-test',
                    },
                },
            } as any
        });
    });

    it('renders the polish button', () => {
        render(<PolishButton />);
        // The button has id="polish-menu-button"
        // In the updated code, data-tooltip is localized key 'polish.title' (default)
        // But with mock, t('polish.title') returns 'polish.title'.
        const button = screen.getByRole('button', { expanded: false });
        expect(button).toBeDefined();
    });

    it('shows start option when clicked', () => {
        render(<PolishButton />);
        const button = screen.getByRole('button', { expanded: false });
        fireEvent.click(button);
        expect(screen.getByText('polish.start')).toBeDefined();
    });

    it('starts polishing when start option clicked', async () => {
        render(<PolishButton />);
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('polish.start'));

        expect(polishService.polishTranscript).toHaveBeenCalled();
    });

    it('shows undo option after polishing (simulated)', async () => {
        render(<PolishButton />);

        // 1. Click LLM Polish
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('polish.start'));

        // Re-open menu
        fireEvent.click(screen.getByRole('button', { expanded: false }));

        expect(screen.getByText('polish.undo')).toBeDefined();
    });

    it('undo restores segments and shows redo', async () => {
        // Setup initial segments
        const initialSegments = [{ id: '1', start: 0, end: 1, text: 'Original', isFinal: true }];
        useTranscriptStore.setState({ segments: initialSegments });

        render(<PolishButton />);

        // 1. LLM Polish (saves 'Original' to undo)
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('polish.start'));

        // Simulate polish changing segments
        const polishedSegments = [{ id: '1', start: 0, end: 1, text: 'Polished', isFinal: true }];
        act(() => {
            useTranscriptStore.setState({ segments: polishedSegments });
        });

        // 2. Undo
        fireEvent.click(screen.getByRole('button', { expanded: false })); // Open menu
        fireEvent.click(screen.getByText('polish.undo')); // Click Undo

        // Verify segments restored to Original
        expect(useTranscriptStore.getState().segments).toEqual(initialSegments);

        // 3. Verify Redo appears
        fireEvent.click(screen.getByRole('button', { expanded: false })); // Open menu
        expect(screen.getByText('polish.redo')).toBeDefined();
    });

    it('redo restores polished segments', async () => {
        // Setup initial segments
        const initialSegments = [{ id: '1', start: 0, end: 1, text: 'Original', isFinal: true }];
        useTranscriptStore.setState({ segments: initialSegments });

        render(<PolishButton />);

        // 1. LLM Polish
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('polish.start'));

        // Simulate polish changing segments
        const polishedSegments = [{ id: '1', start: 0, end: 1, text: 'Polished', isFinal: true }];
        act(() => {
            useTranscriptStore.setState({ segments: polishedSegments });
        });

        // 2. Undo
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('polish.undo'));

        // 3. Redo
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('polish.redo'));

        // Verify segments restored to Polished
        expect(useTranscriptStore.getState().segments).toEqual(polishedSegments);
    });

    it('opens advanced settings modal when clicked', () => {
        render(<PolishButton />);
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('polish.advanced_settings'));

        // Check if modal elements appear (using localized keys from mock)
        expect(screen.getByText('polish.keywords')).toBeDefined();
        expect(screen.getByText('polish.scenario_label')).toBeDefined();
    });
});
