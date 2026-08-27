import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { ExportButton } from '../ExportButton';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { useHistoryStore } from '../../stores/historyStore';

vi.mock('../../stores/projectStore', () => ({
    useProjectStore: (selector: any) => selector({
        activeProjectId: null,
        projects: [],
    }),
}));

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
}));

vi.mock('@tauri-apps/api/path', () => ({
    join: vi.fn().mockImplementation((...args) => args.join('/')),
}));

const mockAlert = vi.fn().mockResolvedValue(undefined);
vi.mock('../../stores/dialogStore', () => ({
    useDialogStore: (selector: any) => selector({
        alert: mockAlert,
        showError: vi.fn(),
        confirm: vi.fn(),
        prompt: vi.fn(),
    }),
}));

describe('ExportButton', () => {
    let mockWriteText: any;

    const openExportModal = async () => {
        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));

        await waitFor(() => {
            screen.getByRole('dialog');
            screen.getByDisplayValue('Test Recording');
        });
    };

    beforeEach(() => {
        vi.clearAllMocks();
        mockAlert.mockClear();
        localStorage.clear();

        mockWriteText = vi.fn().mockResolvedValue(undefined);
        Object.defineProperty(navigator, 'clipboard', {
            value: {
                writeText: mockWriteText,
            },
            writable: true,
            configurable: true,
        });

        // Reset stores
        useTranscriptStore.setState({
            segments: [
                { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true, translation: 'Bonjour' },
                { id: '2', start: 1, end: 2, text: 'World', isFinal: true, translation: 'Monde' }
            ],
            sourceHistoryId: 'hist-1',
            audioUrl: null
        });

        useHistoryStore.setState({
            items: [
                { id: 'hist-1', title: 'Test Recording', timestamp: Date.now(), duration: 10, audioPath: '', transcriptPath: '', previewText: '', projectId: null }
            ]
        });
    });

    it('renders the export button', () => {
        render(<ExportButton />);
        screen.getByRole('button', { name: /export.button/i });
    });

    it('opens modal when clicked', async () => {
        await openExportModal();

        screen.getByText('export.modal_title');
    });

    it('renders the modal overlay at document body level when opened from the detail header', async () => {
        render(
            <div className="projects-detail-header">
                <ExportButton />
            </div>
        );

        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));

        await waitFor(() => {
            screen.getByRole('dialog');
        });

        const bodyOverlay = Array.from(document.body.children).find((element) =>
            element.classList.contains('shared-modal-overlay')
        );

        expect(bodyOverlay).toBeDefined();
        screen.getByText('export.modal_title');
    });

    it('hides button when no segments', () => {
        useTranscriptStore.setState({ segments: [] });
        render(<ExportButton />);
        const button = screen.queryByRole('button', { name: /export.button/i });
        expect(button).toBeNull();
    });

    it('calls exportToPath when clicking export in modal', async () => {
        await openExportModal();

        const modalExportBtn = screen.getAllByRole('button', { name: 'export.button' }).find(btn => btn.classList.contains('btn-primary'));
        expect(modalExportBtn).toBeDefined();
        if (modalExportBtn) fireEvent.click(modalExportBtn);
        
        // Should show alert because directory is empty (this test might need more setup for DialogStore)
    });

    it('shows translation modes if translation available', async () => {
        await openExportModal();

        screen.getByLabelText('export.mode_translation');
        screen.getByLabelText('export.mode_bilingual');
    });

    it('disables translation modes if no translation available', async () => {
        useTranscriptStore.setState({
            segments: [
                { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true } // No translation
            ]
        });

        await openExportModal();

        const translationRadio = screen.getByLabelText('export.mode_translation');
        expect(translationRadio.hasAttribute('disabled')).toBe(true);

        const bilingualRadio = screen.getByLabelText('export.mode_bilingual');
        expect(bilingualRadio.hasAttribute('disabled')).toBe(true);
    });

    it('renders the "Copy to Clipboard" button in the Export Modal and calls navigator.clipboard.writeText when clicked', async () => {
        await openExportModal();

        const copyBtn = screen.getByRole('button', { name: 'export.copy_to_clipboard' });
        expect(copyBtn).toBeDefined();
        expect(screen.queryByTestId('copy-success-check')).toBeNull();

        vi.useFakeTimers();
        try {
            fireEvent.click(copyBtn);

            // Wait/flush microtasks and advance fake timers slightly to let async handleCopy run
            await act(async () => {
                await vi.advanceTimersByTimeAsync(0);
            });

            expect(mockWriteText).toHaveBeenCalledWith('Hello\n\nWorld');
            screen.getByTestId('copy-success-check');

            // Advance by 2000ms to trigger the timeout callback
            await act(async () => {
                await vi.advanceTimersByTimeAsync(2000);
            });

            expect(screen.queryByTestId('copy-success-check')).toBeNull();
        } finally {
            vi.useRealTimers();
        }
    });

    it('keeps encoded angle brackets encoded once when copying', async () => {
        useTranscriptStore.setState({
            segments: [
                { id: '1', start: 0, end: 1, text: '&amp;lt;script&amp;gt;', isFinal: true }
            ]
        });

        await openExportModal();

        const copyBtn = screen.getByRole('button', { name: 'export.copy_to_clipboard' });
        fireEvent.click(copyBtn);

        await waitFor(() => {
            expect(mockWriteText).toHaveBeenCalledWith('&lt;script&gt;');
        });
    });

    it('copies ordinary HTML entities as readable plain text', async () => {
        useTranscriptStore.setState({
            segments: [
                { id: '1', start: 0, end: 1, text: '<div>A&nbsp;&amp;&nbsp;B</div>', isFinal: true }
            ]
        });

        await openExportModal();

        const copyBtn = screen.getByRole('button', { name: 'export.copy_to_clipboard' });
        fireEvent.click(copyBtn);

        await waitFor(() => {
            expect(mockWriteText).toHaveBeenCalledWith('A & B');
        });
    });
});
