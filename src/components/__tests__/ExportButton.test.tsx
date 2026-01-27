import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ExportButton } from '../ExportButton';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: (selector: any) => selector({
        segments: [
            { id: '1', start: 0, end: 1, text: 'Hello world' }
        ],
        audioUrl: 'blob:test'
    })
}));

vi.mock('../../utils/fileExport', () => ({
    saveTranscript: vi.fn(),
}));

describe('ExportButton', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('renders the export button', () => {
        render(<ExportButton />);
        expect(screen.getByText('export.button')).toBeDefined();
    });

    it('has correct ARIA attributes on toggle button', () => {
        render(<ExportButton />);
        const button = screen.getByRole('button', { name: /export/i });

        // Before opening
        expect(button.getAttribute('aria-haspopup')).toBe('true');
        expect(button.getAttribute('aria-expanded')).toBe('false');

        // After opening
        fireEvent.click(button);
        expect(button.getAttribute('aria-expanded')).toBe('true');
    });

    it('renders accessible dropdown menu', () => {
        render(<ExportButton />);
        const button = screen.getByRole('button', { name: /export/i });
        fireEvent.click(button);

        const menu = screen.getByRole('menu');
        expect(menu).toBeDefined();
        expect(menu.getAttribute('aria-labelledby')).toBeDefined();

        const items = screen.getAllByRole('menuitem');
        expect(items.length).toBeGreaterThan(0);
    });
});
