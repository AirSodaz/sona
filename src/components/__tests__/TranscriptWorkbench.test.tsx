import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptWorkbench } from '../TranscriptWorkbench';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { DEFAULT_CONFIG } from '../../stores/configStore';

const mockUpdateItemMeta = vi.fn();

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../stores/historyStore', () => ({
  useHistoryStore: (selector: (state: { updateItemMeta: typeof mockUpdateItemMeta }) => unknown) => selector({
    updateItemMeta: mockUpdateItemMeta,
  }),
}));

vi.mock('../ErrorBoundary', () => ({
  ErrorBoundary: ({ children }: { children: any }) => <>{children}</>,
}));

vi.mock('../TranscriptEditor', () => ({
  TranscriptEditor: () => <div>TranscriptEditor</div>,
}));

vi.mock('../AudioPlayer', () => ({
  AudioPlayer: () => <div>AudioPlayer</div>,
}));

vi.mock('../TranscriptSummaryPanel', () => ({
  TranscriptSummaryPanel: () => null,
}));

vi.mock('../RenameModal', () => ({
  RenameModal: ({ isOpen }: { isOpen: boolean }) => (isOpen ? <div>RenameModal</div> : null),
}));

vi.mock('../../services/aiRenameService', () => ({
  generateAiTitle: vi.fn().mockResolvedValue('AI Title'),
}));

describe('TranscriptWorkbench', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTranscriptStore.setState({
      segments: [
        { id: 'seg-1', start: 0, end: 1, text: 'Hello', isFinal: true },
      ],
      audioUrl: null,
      config: DEFAULT_CONFIG,
      title: 'Transcript Title',
      icon: null,
      sourceHistoryId: null,
      mode: 'live',
      isRecording: false,
      isPaused: false,
    });
  });

  it('keeps rename and close available when no recording session is active', async () => {
    const onClose = vi.fn();
    render(<TranscriptWorkbench onClose={onClose} />);

    const renameButton = screen.getByRole('button', { name: 'common.rename' });
    const closeButton = screen.getByRole('button', { name: 'common.close' });

    expect(renameButton.hasAttribute('disabled')).toBe(false);
    expect(closeButton.hasAttribute('disabled')).toBe(false);

    await act(async () => {
      fireEvent.click(renameButton);
    });

    expect(screen.getByText('RenameModal')).toBeDefined();

    await act(async () => {
      fireEvent.click(closeButton);
    });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('disables rename and close while recording is active', async () => {
    const onClose = vi.fn();
    useTranscriptStore.setState({
      isRecording: true,
      isPaused: false,
    });

    render(<TranscriptWorkbench onClose={onClose} />);

    const renameButton = screen.getByRole('button', { name: 'common.rename' });
    const closeButton = screen.getByRole('button', { name: 'common.close' });

    expect(renameButton.hasAttribute('disabled')).toBe(true);
    expect(closeButton.hasAttribute('disabled')).toBe(true);

    await act(async () => {
      fireEvent.click(renameButton);
      fireEvent.click(closeButton);
    });

    expect(screen.queryByText('RenameModal')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('keeps rename and close disabled while the recording session is paused', async () => {
    const onClose = vi.fn();
    useTranscriptStore.setState({
      isRecording: true,
      isPaused: true,
    });

    render(<TranscriptWorkbench onClose={onClose} />);

    const renameButton = screen.getByRole('button', { name: 'common.rename' });
    const closeButton = screen.getByRole('button', { name: 'common.close' });

    expect(renameButton.hasAttribute('disabled')).toBe(true);
    expect(closeButton.hasAttribute('disabled')).toBe(true);

    await act(async () => {
      fireEvent.click(renameButton);
      fireEvent.click(closeButton);
    });

    expect(screen.queryByText('RenameModal')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('automatically closes the rename modal when recording starts', async () => {
    render(<TranscriptWorkbench onClose={() => undefined} />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'common.rename' }));
    });

    expect(screen.getByText('RenameModal')).toBeDefined();

    await act(async () => {
      useTranscriptStore.setState({
        isRecording: true,
        isPaused: false,
      });
    });

    expect(screen.queryByText('RenameModal')).toBeNull();
    expect(screen.getByRole('button', { name: 'common.rename' }).hasAttribute('disabled')).toBe(true);
  });
});
