import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptWorkbench } from '../TranscriptWorkbench';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
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

vi.mock('../PolishButton', () => ({
  PolishButton: ({ className = '' }: { className?: string }) => (
    <button type="button" className={className} aria-label="PolishButton">PolishButton</button>
  ),
}));

vi.mock('../TranslateButton', () => ({
  TranslateButton: ({ className = '' }: { className?: string }) => (
    <button type="button" className={className} aria-label="TranslateButton">TranslateButton</button>
  ),
}));

vi.mock('../ExportButton', () => ({
  ExportButton: ({ className = '' }: { className?: string }) => (
    <button type="button" className={className} aria-label="ExportButton">ExportButton</button>
  ),
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

  it('renders polish, translate, export, and close in the detail header action group', () => {
    const { container } = render(<TranscriptWorkbench onClose={() => undefined} />);

    const actionGroup = container.querySelector('.projects-detail-header-actions');
    expect(actionGroup).not.toBeNull();

    const actionLabels = Array.from(actionGroup?.querySelectorAll('button') || []).map((button) => (
      button.getAttribute('aria-label') || button.textContent || ''
    ));

    expect(actionLabels).toEqual(['PolishButton', 'TranslateButton', 'ExportButton', 'common.close']);
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
    const polishButton = screen.getByRole('button', { name: 'PolishButton' });
    const translateButton = screen.getByRole('button', { name: 'TranslateButton' });
    const exportButton = screen.getByRole('button', { name: 'ExportButton' });

    expect(renameButton.hasAttribute('disabled')).toBe(true);
    expect(closeButton.hasAttribute('disabled')).toBe(true);
    expect(polishButton.hasAttribute('disabled')).toBe(false);
    expect(translateButton.hasAttribute('disabled')).toBe(false);
    expect(exportButton.hasAttribute('disabled')).toBe(false);

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
