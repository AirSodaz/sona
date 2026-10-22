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

    // Live Tab active
    const liveTab = screen.getByTestId('create-tab-live');
    expect(liveTab.getAttribute('aria-selected')).toBe('true');
    expect(liveTab.classList.contains('active')).toBe(true);
    expect(screen.getByTestId('pane-live').style.display).toBe('');
    expect(screen.getByTestId('live-record-component')).toBeDefined();

    // Batch Tab inactive (body stays mounted for persistent state, but hidden)
    const batchTab = screen.getByTestId('create-tab-batch');
    expect(batchTab.getAttribute('aria-selected')).toBe('false');
    expect(batchTab.classList.contains('active')).toBe(false);
    expect(screen.getByTestId('pane-batch').style.display).toBe('none');
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

    // Batch Tab active
    const batchTab = screen.getByTestId('create-tab-batch');
    expect(batchTab.getAttribute('aria-selected')).toBe('true');
    expect(batchTab.classList.contains('active')).toBe(true);
    expect(screen.getByTestId('pane-batch').style.display).toBe('');
    expect(screen.getByTestId('batch-import-component')).toBeDefined();

    // Live Tab inactive (body stays mounted for background recording, but hidden)
    const liveTab = screen.getByTestId('create-tab-live');
    expect(liveTab.getAttribute('aria-selected')).toBe('false');
    expect(liveTab.classList.contains('active')).toBe(false);
    expect(screen.getByTestId('pane-live').style.display).toBe('none');
    expect(screen.getByTestId('live-record-component')).toBeDefined();
  });

  it('switches ingestionFocus to live when Live tab is clicked', () => {
    testContext.runtimeState.ingestionFocus = 'batch';
    render(<CreateWorkspacePanel />);

    const liveTab = screen.getByTestId('create-tab-live');
    fireEvent.click(liveTab);

    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('live');
  });

  it('renders live-recording badge on live tab when isRecording is true and batch tab is active', () => {
    testContext.runtimeState.ingestionFocus = 'batch';
    testContext.runtimeState.isRecording = true;
    render(<CreateWorkspacePanel />);

    const liveTab = screen.getByTestId('create-tab-live');
    const badge = liveTab.querySelector('.create-tab-badge.live-recording');
    expect(badge).not.toBeNull();
  });

  it('renders batch-count badge on batch tab when queueItems exist and live tab is active', () => {
    testContext.runtimeState.ingestionFocus = 'live';
    testContext.batchQueueState.queueItems = [
      { id: '1', status: 'complete', progress: 100 },
      { id: '2', status: 'processing', progress: 50 },
    ];

    render(<CreateWorkspacePanel />);

    const batchTab = screen.getByTestId('create-tab-batch');
    const badge = batchTab.querySelector('.create-tab-badge.batch-count');
    expect(badge).not.toBeNull();
    expect(badge?.textContent).toBe('2');
  });
});
