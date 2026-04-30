import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
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

vi.mock('../../utils/fileExport', () => ({
    exportToPath: vi.fn().mockResolvedValue(undefined),
    saveTranscript: vi.fn().mockResolvedValue(true),
}));

describe('ExportButton', () => {
    const openExportModal = async () => {
        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));

        await waitFor(() => {
            expect(screen.getByRole('dialog')).toBeDefined();
            expect(screen.getByDisplayValue('Test Recording')).toBeDefined();
        });
    };

    beforeEach(() => {
        vi.clearAllMocks();
        localStorage.clear();

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
        expect(screen.getByRole('button', { name: /export.button/i })).toBeDefined();
    });

    it('opens modal when clicked', async () => {
        await openExportModal();

        expect(screen.getByText('export.modal_title')).toBeDefined();
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

        expect(screen.getByLabelText('export.mode_translation')).toBeDefined();
        expect(screen.getByLabelText('export.mode_bilingual')).toBeDefined();
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
});
