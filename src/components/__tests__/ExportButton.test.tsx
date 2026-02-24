import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ExportButton } from '../ExportButton';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { saveTranscript } from '../../utils/fileExport';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

vi.mock('../../utils/fileExport', () => ({
    saveTranscript: vi.fn().mockResolvedValue(true),
}));

describe('ExportButton', () => {
    beforeEach(() => {
        vi.clearAllMocks();

        // Reset store
        useTranscriptStore.setState({
            segments: [
                { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true, translation: 'Bonjour' },
                { id: '2', start: 1, end: 2, text: 'World', isFinal: true, translation: 'Monde' }
            ],
            audioUrl: null
        });
    });

    it('renders the export button', () => {
        render(<ExportButton />);
        expect(screen.getByText('export.button')).toBeDefined();
    });

    it('opens menu when clicked', () => {
        render(<ExportButton />);
        const button = screen.getByRole('button', { name: /export.button/i });
        fireEvent.click(button);

        expect(screen.getByRole('menu')).toBeDefined();
        expect(screen.getByText('SubRip (.srt)')).toBeDefined();
        // Check for mode selection
        expect(screen.getByText('panel.mode_selection')).toBeDefined();
        expect(screen.getByLabelText('export.mode_original')).toBeDefined();
    });

    it('disables button when no segments', () => {
        useTranscriptStore.setState({ segments: [] });
        render(<ExportButton />);
        const button = screen.getByRole('button', { name: /export.button/i });
        expect(button.hasAttribute('disabled')).toBe(true);
    });

    it('calls saveTranscript with SRT format and default mode', async () => {
        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));
        fireEvent.click(screen.getByText('SubRip (.srt)'));

        expect(saveTranscript).toHaveBeenCalledWith(expect.objectContaining({
            format: 'srt',
            mode: 'original',
        }));
    });

    it('calls saveTranscript with JSON format and Translation mode', async () => {
        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));

        // Select Translation mode
        const translationRadio = screen.getByLabelText('export.mode_translation');
        fireEvent.click(translationRadio);

        fireEvent.click(screen.getByText('JSON (.json)'));

        expect(saveTranscript).toHaveBeenCalledWith(expect.objectContaining({
            format: 'json',
            mode: 'translation',
        }));
    });

    it('calls saveTranscript with Bilingual mode', async () => {
        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));

        // Select Bilingual mode
        const bilingualRadio = screen.getByLabelText('export.mode_bilingual');
        fireEvent.click(bilingualRadio);

        fireEvent.click(screen.getByText('SubRip (.srt)'));

        expect(saveTranscript).toHaveBeenCalledWith(expect.objectContaining({
            format: 'srt',
            mode: 'bilingual',
        }));
    });

    it('disables translation modes if no translation available', async () => {
        useTranscriptStore.setState({
            segments: [
                { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true } // No translation
            ]
        });

        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));

        const translationRadio = screen.getByLabelText('export.mode_translation');
        expect(translationRadio.hasAttribute('disabled')).toBe(true);

        const bilingualRadio = screen.getByLabelText('export.mode_bilingual');
        expect(bilingualRadio.hasAttribute('disabled')).toBe(true);
    });
});
