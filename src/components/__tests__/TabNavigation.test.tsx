import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TabNavigation } from '../TabNavigation';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

const mockSetMode = vi.fn();

vi.mock('../stores/transcriptStore', () => ({
    useTranscriptStore: (selector: any) => selector({
        mode: 'live',
        setMode: mockSetMode,
    })
}));

describe('TabNavigation', () => {
    it('renders with correct ARIA attributes', () => {
        render(<TabNavigation />);

        // Check for tablist role
        const tablist = screen.getByRole('tablist');
        expect(tablist).toBeDefined();
        expect(tablist.getAttribute('aria-label')).toBe('panel.mode_selection');

        // Check for tabs
        const tabs = screen.getAllByRole('tab');
        expect(tabs).toHaveLength(2);

        // Check live tab
        expect(tabs[0].getAttribute('aria-selected')).toBe('true');
        expect(tabs[0].textContent).toContain('panel.live_record');

        // Check batch tab
        expect(tabs[1].getAttribute('aria-selected')).toBe('false');
        expect(tabs[1].textContent).toContain('panel.batch_import');
    });
});
