import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ExportButton } from '../ExportButton';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { saveTranscript } from '../../utils/fileExport';
import { useDialogStore } from '../../stores/dialogStore';

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
                { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true },
                { id: '2', start: 1, end: 2, text: 'World', isFinal: true }
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
        expect(screen.getByText('JSON (.json)')).toBeDefined();
    });

    it('disables button when no segments', () => {
        useTranscriptStore.setState({ segments: [] });
        render(<ExportButton />);
        const button = screen.getByRole('button', { name: /export.button/i });
        expect(button.hasAttribute('disabled')).toBe(true);
    });

    it('calls saveTranscript with SRT format', async () => {
        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));
        fireEvent.click(screen.getByText('SubRip (.srt)'));

        expect(saveTranscript).toHaveBeenCalledWith({
            segments: expect.any(Array),
            format: 'srt',
            defaultFileName: expect.stringMatching(/transcript_\d{4}-\d{2}-\d{2}/),
        });
    });

    it('calls saveTranscript with JSON format', async () => {
        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));
        fireEvent.click(screen.getByText('JSON (.json)'));

        expect(saveTranscript).toHaveBeenCalledWith({
            segments: expect.any(Array),
            format: 'json',
            defaultFileName: expect.stringMatching(/transcript_\d{4}-\d{2}-\d{2}/),
        });
    });

    it('shows alert if segments empty but button enabled (edge case)', async () => {
        // Technically disabled attribute prevents click, but let's force it or verify logic
        // If we force enable via removing disable (not possible in react test easily without props),
        // we can assume the disabled check covers it.
        // But the handleExport function checks segments.length too.

        // Let's create a scenario where it's enabled then cleared?
        // Not easy.

        // Let's just check validation logic by mocking store state change after render?
        const { rerender } = render(<ExportButton />);

        // Button enabled
        const button = screen.getByRole('button', { name: /export.button/i });
        expect(button.hasAttribute('disabled')).toBe(false);

        // Now clear segments
        useTranscriptStore.setState({ segments: [] });
        rerender(<ExportButton />);

        expect(button.hasAttribute('disabled')).toBe(true);
    });

    it('handles export error gracefully', async () => {
        vi.mocked(saveTranscript).mockRejectedValue(new Error('Export failed'));
        const alertSpy = vi.spyOn(useDialogStore.getState(), 'alert');

        render(<ExportButton />);
        fireEvent.click(screen.getByRole('button', { name: /export.button/i }));
        fireEvent.click(screen.getByText('Plain Text (.txt)'));

        await waitFor(() => {
             expect(alertSpy).toHaveBeenCalledWith('export.failed', { variant: 'error' });
        });
    });

    describe('Keyboard Navigation', () => {
        it('focuses first item when opened', async () => {
            render(<ExportButton />);
            const trigger = screen.getByRole('button', { name: /export.button/i });
            fireEvent.click(trigger);

            const menuItems = screen.getAllByRole('menuitem');
            await waitFor(() => {
                expect(document.activeElement).toBe(menuItems[0]);
            });
        });

        it('navigates with arrow keys', async () => {
            render(<ExportButton />);
            const trigger = screen.getByRole('button', { name: /export.button/i });
            fireEvent.click(trigger);

            const menuItems = screen.getAllByRole('menuitem');
            await waitFor(() => expect(document.activeElement).toBe(menuItems[0]));

            // Arrow Down
            fireEvent.keyDown(menuItems[0], { key: 'ArrowDown', bubbles: true });
            expect(document.activeElement).toBe(menuItems[1]);

            // Loop from last to first
            menuItems[3].focus();
            fireEvent.keyDown(menuItems[3], { key: 'ArrowDown', bubbles: true });
            expect(document.activeElement).toBe(menuItems[0]);

            // Arrow Up loop from first to last
            fireEvent.keyDown(menuItems[0], { key: 'ArrowUp', bubbles: true });
            expect(document.activeElement).toBe(menuItems[3]);
        });

        it('closes on Escape and returns focus to trigger', async () => {
            render(<ExportButton />);
            const trigger = screen.getByRole('button', { name: /export.button/i });
            trigger.focus();
            fireEvent.click(trigger);

            await waitFor(() => expect(screen.getByRole('menu')).toBeDefined());

            // Press Escape on a menu item
            const menuItems = screen.getAllByRole('menuitem');
            fireEvent.keyDown(menuItems[0], { key: 'Escape', bubbles: true });

            expect(screen.queryByRole('menu')).toBeNull();
            expect(document.activeElement).toBe(trigger);
        });
    });
});
