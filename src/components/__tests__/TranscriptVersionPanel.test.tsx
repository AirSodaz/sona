import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptVersionPanel } from '../TranscriptVersionPanel';
import { transcriptSnapshotService } from '../../services/transcriptSnapshotService';
import { useHistoryStore } from '../../stores/historyStore';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../test-utils/transcriptStoreTestUtils';

const confirmMock = vi.fn();
const showErrorMock = vi.fn();
const tMock = vi.fn((key: string, options?: Record<string, unknown>) => (
  typeof options?.count === 'number' ? `${key}:${options.count}` : key
));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: tMock,
  }),
}));

vi.mock('../../services/transcriptSnapshotService', () => ({
  transcriptSnapshotService: {
    buildDiff: vi.fn(),
    createSnapshot: vi.fn(),
    listSnapshots: vi.fn(),
    loadSnapshot: vi.fn(),
    restoreDiffRows: vi.fn(),
  },
}));

vi.mock('../../stores/dialogStore', () => ({
  useDialogStore: (selector: (state: { confirm: typeof confirmMock; showError: typeof showErrorMock }) => unknown) => selector({
    confirm: confirmMock,
    showError: showErrorMock,
  }),
}));

describe('TranscriptVersionPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTranscriptStores();
    confirmMock.mockResolvedValue(true);
    vi.mocked(transcriptSnapshotService.createSnapshot).mockResolvedValue(null);
    vi.mocked(transcriptSnapshotService.buildDiff).mockResolvedValue({ rows: [], changedCount: 0 });
    vi.mocked(transcriptSnapshotService.restoreDiffRows).mockResolvedValue([]);
    useHistoryStore.setState({ updateTranscript: vi.fn().mockResolvedValue(undefined) } as Partial<ReturnType<typeof useHistoryStore.getState>>);
  });

  it('renders an empty snapshot state', async () => {
    vi.mocked(transcriptSnapshotService.listSnapshots).mockResolvedValue([]);

    const { container } = render(<TranscriptVersionPanel isOpen historyId="history-a" onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('versions.empty')).toBeDefined();
    });

    expect(container.querySelector('.panel-modal-shell.transcript-version-modal')).toBeTruthy();
    expect(container.querySelector('.panel-modal-shell.panel-modal-size-settings.transcript-version-modal')).toBeTruthy();
    expect(container.querySelector('.panel-modal-header.transcript-version-header')).toBeTruthy();
    expect(container.querySelector('.panel-modal-badge.transcript-version-badge')).toBeTruthy();
    expect(container.querySelector('.panel-modal-toolbar.transcript-version-actions')).toBeTruthy();
    expect(container.querySelector('.panel-modal-meta-row.transcript-version-meta-row')).toBeTruthy();
    expect(container.querySelector('.panel-modal-content.transcript-version-content')).toBeTruthy();
  });

  it('restores selected diff rows and snapshots current content first', async () => {
    const updateTranscript = vi.fn().mockResolvedValue(undefined);
    const currentSegment = { id: 'seg-1', start: 0, end: 1, text: 'new', isFinal: true };
    const snapshotSegment = { id: 'seg-1', start: 0, end: 1, text: 'old', isFinal: true };
    useHistoryStore.setState({ updateTranscript } as Partial<ReturnType<typeof useHistoryStore.getState>>);
    useTranscriptStore.setState({
      sourceHistoryId: 'history-a',
      segments: [currentSegment],
    });
    vi.mocked(transcriptSnapshotService.listSnapshots).mockResolvedValue([
      {
        id: 'snapshot-1',
        historyId: 'history-a',
        reason: 'polish',
        createdAt: 1,
        segmentCount: 1,
      },
    ]);
    vi.mocked(transcriptSnapshotService.loadSnapshot).mockResolvedValue({
      metadata: {
        id: 'snapshot-1',
        historyId: 'history-a',
        reason: 'polish',
        createdAt: 1,
        segmentCount: 1,
      },
      segments: [snapshotSegment],
    });
    vi.mocked(transcriptSnapshotService.buildDiff).mockResolvedValue({
      changedCount: 1,
      rows: [{
        id: 'diff-0-0',
        status: 'modified',
        snapshotSegment,
        currentSegment,
        snapshotIndex: 0,
        currentIndex: 0,
      }],
    });
    vi.mocked(transcriptSnapshotService.restoreDiffRows).mockResolvedValue([snapshotSegment]);

    render(<TranscriptVersionPanel isOpen historyId="history-a" onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('old')).toBeDefined();
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'versions.diff.modified' }));
    });
    await act(async () => {
      fireEvent.click(screen.getByText('versions.restore_selected'));
    });

    expect(transcriptSnapshotService.createSnapshot).toHaveBeenCalledWith('history-a', 'restore', [
      expect.objectContaining({ text: 'new' }),
    ]);
    expect(transcriptSnapshotService.restoreDiffRows).toHaveBeenCalledWith([
      expect.objectContaining({ id: 'diff-0-0', status: 'modified' }),
    ], expect.any(Set));
    expect(updateTranscript).toHaveBeenCalledWith('history-a', [
      expect.objectContaining({ text: 'old' }),
    ]);
  });
});
