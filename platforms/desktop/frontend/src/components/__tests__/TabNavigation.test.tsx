import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const testContext = vi.hoisted(() => ({
  runtimeState: {
    mode: 'create' as 'create' | 'projects' | 'tools',
    setMode: vi.fn(),
  },
  transcriptState: {
    ingestionFocus: 'live' as 'live' | 'batch',
    recordingSessionId: null as string | null,
    sessions: {} as Record<string, unknown>,
  },
}));

// Mock dependencies
vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    }),
  };
});

vi.mock('../../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: <T,>(selector: (state: typeof testContext.runtimeState) => T) =>
    selector(testContext.runtimeState),
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: {
    getState: () => testContext.transcriptState,
    setState: vi.fn(),
  },
}));

import { TabNavigation } from '../TabNavigation';

describe('TabNavigation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    testContext.runtimeState.mode = 'create';
    testContext.transcriptState.ingestionFocus = 'live';
  });
  afterEach(() => {
    cleanup();
  });

  it('renders with correct ARIA attributes', () => {
    render(<TabNavigation />);

    // Check for tablist role
    const tablist = screen.getByRole('tablist');
    expect(tablist).toBeDefined();
    expect(tablist.getAttribute('aria-label')).toBe('Mode Selection');

    // Check for tabs
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(3);

    // Check create tab
    expect(tabs[0].getAttribute('aria-selected')).toBe('true');
    expect(tabs[0].textContent).toContain('Create');
    expect(tabs[0].hasAttribute('data-tooltip')).toBe(false);
    expect(tabs[0].querySelector('svg')).not.toBeNull();

    // Check workspace tab
    expect(tabs[1].getAttribute('aria-selected')).toBe('false');
    expect(tabs[1].textContent).toContain('Projects');
    expect(tabs[1].hasAttribute('data-tooltip')).toBe(false);

    // Check tools tab
    expect(tabs[2].getAttribute('aria-selected')).toBe('false');
    expect(tabs[2].textContent).toContain('Tools');
    expect(tabs[2].hasAttribute('data-tooltip')).toBe(false);
    expect(tabs[0].getAttribute('tabindex')).toBe('0');
    expect(tabs[1].getAttribute('tabindex')).toBe('-1');
    expect(tabs[2].getAttribute('tabindex')).toBe('-1');
    expect(tabs[2].hasAttribute('data-tooltip')).toBe(false);
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
      fireEvent.click(tabs[1]); // projects is index 1
      expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
      expect(mainScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
      expect(railScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
    } finally {
      mainScroll.remove();
      railList.remove();
    }
  });

  it('scrolls batch queue to top when active Create tab is clicked and ingestionFocus is batch', () => {
    testContext.runtimeState.mode = 'create';
    testContext.transcriptState.ingestionFocus = 'batch';

    const queueList = document.createElement('div');
    queueList.className = 'queue-list';
    const queueScrollTo = vi.fn();
    queueList.scrollTo = queueScrollTo;
    document.body.append(queueList);

    try {
      render(<TabNavigation />);

      const tabs = screen.getAllByRole('tab');
      fireEvent.click(tabs[0]); // create is index 0

      expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
      expect(queueScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
    } finally {
      queueList.remove();
    }
  });
  it('scrolls tools container to top when active Tools tab is clicked', () => {
    testContext.runtimeState.mode = 'tools';
    const detailPane = document.createElement('div');
    detailPane.className = 'tools-detail-pane';
    const detailScrollTo = vi.fn();
    detailPane.scrollTo = detailScrollTo;
    document.body.append(detailPane);

    try {
      render(<TabNavigation />);

      const tabs = screen.getAllByRole('tab');
      fireEvent.click(tabs[2]); // tools is index 2
      expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
      expect(detailScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });
    } finally {
      detailPane.remove();
    }
  });

  it('cycles through 3 modes on Ctrl+Tab and Ctrl+Shift+Tab', () => {
    testContext.runtimeState.mode = 'projects';
    render(<TabNavigation />);

    // Forward: projects (index 1) -> tools (index 2)
    fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('tools');

    // Backward: projects (index 1) -> create (index 0)
    fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true, shiftKey: true });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('create');
  });

  it('wraps around boundaries on Ctrl+Tab and Ctrl+Shift+Tab', () => {
    // Forward wrap: tools (index 2) -> create (index 0)
    testContext.runtimeState.mode = 'tools';
    const { unmount } = render(<TabNavigation />);

    fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('create');

    unmount();
    testContext.runtimeState.setMode.mockClear();

    // Backward wrap: create (index 0) -> tools (index 2)
    testContext.runtimeState.mode = 'create';
    render(<TabNavigation />);

    fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true, shiftKey: true });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('tools');
  });

  it('navigates tabs with horizontal arrow keys (ArrowRight, ArrowLeft, Home, End)', () => {
    testContext.runtimeState.mode = 'create';
    const { unmount } = render(<TabNavigation />);
    const tablist = screen.getByRole('tablist');

    fireEvent.keyDown(tablist, { key: 'ArrowRight' });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('projects');

    fireEvent.keyDown(tablist, { key: 'ArrowLeft' });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('tools');

    fireEvent.keyDown(tablist, { key: 'End' });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('tools');

    unmount();
    testContext.runtimeState.setMode.mockClear();
    testContext.runtimeState.mode = 'tools';
    render(<TabNavigation />);
    const tablistTools = screen.getByRole('tablist');

    fireEvent.keyDown(tablistTools, { key: 'Home' });
    expect(testContext.runtimeState.setMode).toHaveBeenCalledWith('create');
  });

  it('suppresses Ctrl+Tab when settings or panel modal overlay is open', () => {
    testContext.runtimeState.mode = 'create';
    const overlay = document.createElement('div');
    overlay.className = 'panel-modal-overlay';
    document.body.appendChild(overlay);

    try {
      render(<TabNavigation />);
      fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
      expect(testContext.runtimeState.setMode).not.toHaveBeenCalled();
    } finally {
      overlay.remove();
    }
  });
});
