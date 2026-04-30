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
    createSnapshot: vi.fn(),
    listSnapshots: vi.fn(),
    loadSnapshot: vi.fn(),
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
    useHistoryStore.setState({ updateTranscript: vi.fn().mockResolvedValue(undefined) } as Partial<ReturnType<typeof useHistoryStore.getState>>);
  });

  it('renders an empty snapshot state', async () => {
    vi.mocked(transcriptSnapshotService.listSnapshots).mockResolvedValue([]);

    render(<TranscriptVersionPanel isOpen historyId="history-a" onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('versions.empty')).toBeDefined();
    });
  });

  it('restores selected diff rows and snapshots current content first', async () => {
    const updateTranscript = vi.fn().mockResolvedValue(undefined);
    useHistoryStore.setState({ updateTranscript } as Partial<ReturnType<typeof useHistoryStore.getState>>);
    useTranscriptStore.setState({
      sourceHistoryId: 'history-a',
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'new', isFinal: true }],
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
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'old', isFinal: true }],
    });

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
    expect(updateTranscript).toHaveBeenCalledWith('history-a', [
      expect.objectContaining({ text: 'old' }),
    ]);
  });
});
