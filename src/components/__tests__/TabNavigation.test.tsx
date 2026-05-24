import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';

const testContext = vi.hoisted(() => ({
    runtimeState: {
        mode: 'live',
        setMode: vi.fn(),
    }
}));

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

vi.mock('../../stores/transcriptRuntimeStore', () => ({
    useTranscriptRuntimeStore: (selector: any) => selector(testContext.runtimeState)
}));

import { TabNavigation } from '../TabNavigation';

describe('TabNavigation', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        testContext.runtimeState.mode = 'live';
    });

    afterEach(() => {
        cleanup();
    });

    it('renders with correct ARIA attributes', () => {
        render(<TabNavigation />);

        // Check for tablist role
        const tablist = screen.getByRole('tablist');
        expect(tablist).toBeDefined();
        expect(tablist.getAttribute('aria-label')).toBe('panel.mode_selection');

        // Check for tabs
        const tabs = screen.getAllByRole('tab');
        expect(tabs).toHaveLength(3);

        // Check live tab
        expect(tabs[0].getAttribute('aria-selected')).toBe('true');
        expect(tabs[0].textContent).toContain('panel.live_record');

        // Check batch tab
        expect(tabs[1].getAttribute('aria-selected')).toBe('false');
        expect(tabs[1].textContent).toContain('panel.batch_import');

        // Check workspace tab
        expect(tabs[2].getAttribute('aria-selected')).toBe('false');
        expect(tabs[2].textContent).toContain('panel.projects');
    });

    it('scrolls projects containers to top when active Projects tab is clicked', () => {
        testContext.runtimeState.mode = 'projects';
        const mockScrollTo = vi.fn();
        const mockElement = { scrollTo: mockScrollTo };
        const querySelectorSpy = vi.spyOn(document, 'querySelector').mockReturnValue(mockElement as any);

        render(<TabNavigation />);

        const tabs = screen.getAllByRole('tab');
        const projectsTab = tabs[2]; // projects is index 2

        fireEvent.click(projectsTab);

        expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
        expect(querySelectorSpy).toHaveBeenCalledWith('.projects-main-scroll');
        expect(querySelectorSpy).toHaveBeenCalledWith('.projects-rail-list');
        expect(mockScrollTo).toHaveBeenCalledTimes(2);
        expect(mockScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });

        querySelectorSpy.mockRestore();
    });

    it('scrolls batch queue to top when active Batch Import tab is clicked', () => {
        testContext.runtimeState.mode = 'batch';
        const mockScrollTo = vi.fn();
        const mockElement = { scrollTo: mockScrollTo };
        const querySelectorSpy = vi.spyOn(document, 'querySelector').mockReturnValue(mockElement as any);

        render(<TabNavigation />);

        const tabs = screen.getAllByRole('tab');
        const batchTab = tabs[1]; // batch is index 1

        fireEvent.click(batchTab);

        expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
        expect(querySelectorSpy).toHaveBeenCalledWith('.queue-list');
        expect(mockScrollTo).toHaveBeenCalledTimes(1);
        expect(mockScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });

        querySelectorSpy.mockRestore();
    });
});

