import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { IngestionFocus } from '../../types/transcript';

const testContext = vi.hoisted(() => ({
  runtimeState: {
    ingestionFocus: 'live' as IngestionFocus,
    isRecording: false,
    isPaused: false,
  },
  transcriptState: {
    setIngestionFocus: vi.fn(),
  },
  batchQueueState: {
    queueItems: [] as Array<{ id: string; status: string; progress: number }>,
    isQueueProcessing: false,
  },
}));

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
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
  useTranscriptStore: <T,>(selector: (state: typeof testContext.transcriptState) => T) =>
    selector(testContext.transcriptState),
}));

vi.mock('../../stores/batchQueueStore', () => ({
  useBatchQueueStore: <T,>(selector: (state: typeof testContext.batchQueueState) => T) =>
    selector(testContext.batchQueueState),
}));

vi.mock('../LiveRecord', () => ({
  LiveRecord: () => <div data-testid="live-record-component">LiveRecord Component</div>,
}));

vi.mock('../BatchImport', () => ({
  BatchImport: () => <div data-testid="batch-import-component">BatchImport Component</div>,
}));

import { CreateWorkspacePanel } from '../CreateWorkspacePanel';

describe('CreateWorkspacePanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    testContext.runtimeState.ingestionFocus = 'live';
    testContext.runtimeState.isRecording = false;
    testContext.runtimeState.isPaused = false;
    testContext.batchQueueState.queueItems = [];
    testContext.batchQueueState.isQueueProcessing = false;
  });

  afterEach(() => {
    cleanup();
  });

  it('renders default state with Live tab active and Batch pane hidden', () => {
    render(<CreateWorkspacePanel />);

    const shell = screen.getByTestId('create-workspace-shell');
    expect(shell.className).toContain('state-live');

    const tablist = screen.getByRole('tablist', { name: 'Mode Selection' });
    expect(tablist).not.toBeNull();

    // Slider and Live Tab active
    const slider = shell.querySelector('.create-tab-slider') as HTMLElement;
    expect(slider).not.toBeNull();
    expect(slider.style.transform).toBe('translateX(calc(0 * (100% + 4px)))');

    const liveTab = screen.getByTestId('create-tab-live');
    expect(liveTab.getAttribute('aria-selected')).toBe('true');
    expect(liveTab.getAttribute('aria-controls')).toBe('pane-live');
    expect(liveTab.getAttribute('tabindex')).toBe('0');
    expect(liveTab.classList.contains('active')).toBe(true);

    const livePane = screen.getByTestId('pane-live');
    expect(livePane.getAttribute('role')).toBe('tabpanel');
    expect(livePane.getAttribute('aria-labelledby')).toBe('create-tab-live');
    expect(livePane.getAttribute('tabindex')).toBe('0');
    expect(livePane.style.display).toBe('');
    expect(screen.getByTestId('live-record-component')).toBeDefined();

    // Batch Tab inactive (body stays mounted for persistent state, but hidden)
    const batchTab = screen.getByTestId('create-tab-batch');
    expect(batchTab.getAttribute('aria-selected')).toBe('false');
    expect(batchTab.getAttribute('aria-controls')).toBe('pane-batch');
    expect(batchTab.getAttribute('tabindex')).toBe('-1');
    expect(batchTab.classList.contains('active')).toBe(false);

    const batchPane = screen.getByTestId('pane-batch');
    expect(batchPane.getAttribute('role')).toBe('tabpanel');
    expect(batchPane.getAttribute('aria-labelledby')).toBe('create-tab-batch');
    expect(batchPane.getAttribute('tabindex')).toBe('0');
    expect(batchPane.style.display).toBe('none');
    expect(screen.getByTestId('batch-import-component')).toBeDefined();
  });

  it('switches ingestionFocus to batch when Batch tab is clicked', () => {
    render(<CreateWorkspacePanel />);

    const batchTab = screen.getByTestId('create-tab-batch');
    fireEvent.click(batchTab);

    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('batch');
  });

  it('renders batch state with Batch tab active and Live pane hidden', () => {
    testContext.runtimeState.ingestionFocus = 'batch';
    render(<CreateWorkspacePanel />);

    const shell = screen.getByTestId('create-workspace-shell');
    expect(shell.className).toContain('state-batch');

    // Slider and Batch Tab active
    const slider = shell.querySelector('.create-tab-slider') as HTMLElement;
    expect(slider).not.toBeNull();
    expect(slider.style.transform).toBe('translateX(calc(1 * (100% + 4px)))');

    const batchTab = screen.getByTestId('create-tab-batch');
    expect(batchTab.getAttribute('aria-selected')).toBe('true');
    expect(batchTab.getAttribute('aria-controls')).toBe('pane-batch');
    expect(batchTab.getAttribute('tabindex')).toBe('0');
    expect(batchTab.classList.contains('active')).toBe(true);

    const batchPane = screen.getByTestId('pane-batch');
    expect(batchPane.getAttribute('role')).toBe('tabpanel');
    expect(batchPane.getAttribute('aria-labelledby')).toBe('create-tab-batch');
    expect(batchPane.getAttribute('tabindex')).toBe('0');
    expect(batchPane.style.display).toBe('');
    expect(screen.getByTestId('batch-import-component')).toBeDefined();

    // Live Tab inactive (body stays mounted for background recording, but hidden)
    const liveTab = screen.getByTestId('create-tab-live');
    expect(liveTab.getAttribute('aria-selected')).toBe('false');
    expect(liveTab.getAttribute('aria-controls')).toBe('pane-live');
    expect(liveTab.getAttribute('tabindex')).toBe('-1');
    expect(liveTab.classList.contains('active')).toBe(false);

    const livePane = screen.getByTestId('pane-live');
    expect(livePane.getAttribute('role')).toBe('tabpanel');
    expect(livePane.getAttribute('aria-labelledby')).toBe('create-tab-live');
    expect(livePane.getAttribute('tabindex')).toBe('0');
    expect(livePane.style.display).toBe('none');
    expect(screen.getByTestId('live-record-component')).toBeDefined();
  });

  it('switches ingestionFocus to live when Live tab is clicked', () => {
    testContext.runtimeState.ingestionFocus = 'batch';
    render(<CreateWorkspacePanel />);

    const liveTab = screen.getByTestId('create-tab-live');
    fireEvent.click(liveTab);

    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('live');
  });

  it('renders live-recording badge and sr-only label on live tab when isRecording is true and batch tab is active', () => {
    testContext.runtimeState.ingestionFocus = 'batch';
    testContext.runtimeState.isRecording = true;
    render(<CreateWorkspacePanel />);

    const liveTab = screen.getByTestId('create-tab-live');
    const badge = liveTab.querySelector('.create-tab-badge.live-recording');
    expect(badge).not.toBeNull();
    const srOnly = liveTab.querySelector('.sr-only');
    expect(srOnly).not.toBeNull();
    expect(srOnly?.textContent).toContain('Recording');
  });

  it('renders batch-count badge on batch tab when queueItems exist and are not processing', () => {
    testContext.runtimeState.ingestionFocus = 'live';
    testContext.batchQueueState.isQueueProcessing = false;
    testContext.batchQueueState.queueItems = [
      { id: '1', status: 'complete', progress: 100 },
      { id: '2', status: 'complete', progress: 100 },
    ];

    render(<CreateWorkspacePanel />);

    const batchTab = screen.getByTestId('create-tab-batch');
    const badge = batchTab.querySelector('.create-tab-badge.batch-count');
    expect(badge).not.toBeNull();
    expect(badge?.querySelector('[aria-hidden="true"]')?.textContent).toBe('2');
    const srOnly = badge?.querySelector('.sr-only');
    expect(srOnly).not.toBeNull();
    expect(srOnly?.textContent).toContain('2 items');
  });

  it('renders batch-processing blue dot badge on batch tab when batch items are processing and live tab is active', () => {
    testContext.runtimeState.ingestionFocus = 'live';
    testContext.batchQueueState.isQueueProcessing = true;
    testContext.batchQueueState.queueItems = [{ id: '1', status: 'processing', progress: 50 }];

    render(<CreateWorkspacePanel />);

    const batchTab = screen.getByTestId('create-tab-batch');
    const badge = batchTab.querySelector('.create-tab-badge.batch-processing');
    expect(badge).not.toBeNull();
    const srOnly = batchTab.querySelector('.sr-only');
    expect(srOnly).not.toBeNull();
    expect(srOnly?.textContent).toContain('Processing');
  });

  it('supports arrow key navigation to switch between tabs and roving focus', () => {
    render(<CreateWorkspacePanel />);

    const tablist = screen.getByRole('tablist');
    const liveTab = screen.getByTestId('create-tab-live');
    const batchTab = screen.getByTestId('create-tab-batch');

    liveTab.focus();
    expect(document.activeElement).toBe(liveTab);

    fireEvent.keyDown(tablist, { key: 'ArrowRight' });
    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('batch');
    expect(document.activeElement).toBe(batchTab);

    fireEvent.keyDown(tablist, { key: 'ArrowDown' });
    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('batch');

    testContext.transcriptState.setIngestionFocus.mockClear();

    fireEvent.keyDown(tablist, { key: 'ArrowLeft' });
    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('live');
    expect(document.activeElement).toBe(liveTab);

    fireEvent.keyDown(tablist, { key: 'ArrowUp' });
    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('live');

    testContext.transcriptState.setIngestionFocus.mockClear();

    fireEvent.keyDown(tablist, { key: 'End' });
    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('batch');
    expect(document.activeElement).toBe(batchTab);

    testContext.transcriptState.setIngestionFocus.mockClear();

    fireEvent.keyDown(tablist, { key: 'Home' });
    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('live');
    expect(document.activeElement).toBe(liveTab);
  });
});
