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
        const mainScroll = document.createElement('div');
        mainScroll.className = 'projects-main-scroll';
        const mainScrollTo = vi.fn();
        mainScroll.scrollTo = mainScrollTo;
        const railList = document.createElement('div');
        railList.className = 'projects-rail-list';
        const railScrollTo = vi.fn();
        railList.scrollTo = railScrollTo;
        document.body.append(mainScroll, railList);

        try {
            render(<TabNavigation />);

            const tabs = screen.getAllByRole('tab');
            fireEvent.click(tabs[2]); // projects is index 2

            expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
            expect(mainScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
            expect(railScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
        } finally {
            mainScroll.remove();
            railList.remove();
        }
    });

    it('scrolls batch queue to top when active Batch Import tab is clicked', () => {
        testContext.runtimeState.mode = 'batch';
        const queueList = document.createElement('div');
        queueList.className = 'queue-list';
        const queueScrollTo = vi.fn();
        queueList.scrollTo = queueScrollTo;
        document.body.append(queueList);

        try {
            render(<TabNavigation />);

            const tabs = screen.getAllByRole('tab');
            fireEvent.click(tabs[1]); // batch is index 1

            expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
            expect(queueScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
        } finally {
            queueList.remove();
        }
    });
});
