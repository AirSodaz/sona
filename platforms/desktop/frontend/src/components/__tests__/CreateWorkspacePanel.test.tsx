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

  it('renders default state with Live Deck expanded and Batch Deck collapsed', () => {
    render(<CreateWorkspacePanel />);

    const shell = screen.getByTestId('create-workspace-shell');
    expect(shell.className).toContain('state-live');

    // Live Deck expanded
    expect(screen.getByTestId('deck-live-header')).toBeDefined();
    expect(screen.getByTestId('deck-body-live').style.display).toBe('');
    expect(screen.getByTestId('live-record-component')).toBeDefined();

    // Batch Deck collapsed (body stays mounted for persistent state, but hidden)
    expect(screen.getByTestId('deck-batch-collapsed-bar')).toBeDefined();
    expect(screen.getByTestId('deck-body-batch').style.display).toBe('none');
    expect(screen.getByTestId('batch-import-component')).toBeDefined();

    // Must NOT contain "展开" or "收起" text
    expect(screen.queryByText(/展开/)).toBeNull();
    expect(screen.queryByText(/收起/)).toBeNull();
  });

  it('switches ingestionFocus to batch when collapsed Batch Deck is clicked', () => {
    render(<CreateWorkspacePanel />);

    const batchCollapsedBar = screen.getByTestId('deck-batch-collapsed-bar');
    fireEvent.click(batchCollapsedBar);

    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('batch');
  });

  it('renders batch state with Batch Deck expanded and Live Deck collapsed', () => {
    testContext.runtimeState.ingestionFocus = 'batch';
    testContext.runtimeState.isRecording = true;
    render(<CreateWorkspacePanel />);

    const shell = screen.getByTestId('create-workspace-shell');
    expect(shell.className).toContain('state-batch');

    // Live Deck collapsed (body stays mounted for background recording, but hidden)
    expect(screen.getByTestId('deck-live-collapsed-bar')).toBeDefined();
    expect(screen.getByTestId('deck-body-live').style.display).toBe('none');
    expect(screen.getByTestId('live-record-component')).toBeDefined();
    expect(screen.getByText('录音中')).toBeDefined();

    // Batch Deck expanded
    expect(screen.getByTestId('deck-batch-header')).toBeDefined();
    expect(screen.getByTestId('deck-body-batch').style.display).toBe('');
    expect(screen.getByTestId('batch-import-component')).toBeDefined();

    // Must NOT contain "展开" or "收起" text
    expect(screen.queryByText(/展开/)).toBeNull();
    expect(screen.queryByText(/收起/)).toBeNull();
  });

  it('switches ingestionFocus to live when collapsed Live Deck is clicked', () => {
    testContext.runtimeState.ingestionFocus = 'batch';
    render(<CreateWorkspacePanel />);

    const liveCollapsedBar = screen.getByTestId('deck-live-collapsed-bar');
    fireEvent.click(liveCollapsedBar);

    expect(testContext.transcriptState.setIngestionFocus).toHaveBeenCalledWith('live');
  });

  it('displays summary and mini progress bar in collapsed batch deck when tasks exist', () => {
    testContext.runtimeState.ingestionFocus = 'live';
    testContext.batchQueueState.queueItems = [
      { id: '1', status: 'complete', progress: 100 },
      { id: '2', status: 'processing', progress: 50 },
    ];

    const { container } = render(<CreateWorkspacePanel />);

    const progressBar = container.querySelector('.deck-mini-progress');
    expect(progressBar).toBeDefined();
    expect(progressBar?.getAttribute('aria-hidden')).toBe('true');
    expect(screen.getByText('2 个文件 · 75%')).toBeDefined();
  });
});
