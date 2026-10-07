import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createReactI18nextMock } from '../../__tests__/testUtils/i18n';
import { useDialogStore } from '../../stores/dialogStore';
import type { BatchQueueItem } from '../../types/batchQueue';
import { QueueItem } from '../QueueItem';

vi.mock('react-i18next', () => createReactI18nextMock());

describe('QueueItem', () => {
  const defaultT = (key: string, options?: any) => {
    if (key === 'batch.cancel_confirm_message') {
      return `Stop ${options?.filename} (${options?.progress}%)?`;
    }
    if (key === 'common.delete_item' && options?.item) {
      return `Delete ${options.item}`;
    }
    return key;
  };
  const createItem = (overrides?: Partial<BatchQueueItem>): BatchQueueItem => ({
    id: 'item-1',
    filename: 'audio.mp3',
    filePath: '/path/audio.mp3',
    status: 'pending',
    progress: 0,
    segments: [],
    projectId: null,
    ...overrides,
  });

  beforeEach(() => {
    vi.clearAllMocks();
    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
  });

  it('removes non-processing item immediately without confirmation', () => {
    const onRemove = vi.fn();
    const confirmSpy = vi.spyOn(useDialogStore.getState(), 'confirm');
    const item = createItem({ status: 'pending' });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={vi.fn()}
        onRemove={onRemove}
        t={defaultT as any}
      />
    );

    const removeBtn = screen.getByRole('button', { name: 'Delete audio.mp3' });
    fireEvent.click(removeBtn);

    expect(confirmSpy).not.toHaveBeenCalled();
    expect(onRemove).toHaveBeenCalledWith('item-1');
  });

  it('prompts confirmation when removing a processing item and aborts if declined', async () => {
    const onRemove = vi.fn();
    const confirmSpy = vi.spyOn(useDialogStore.getState(), 'confirm').mockResolvedValue(false);
    const item = createItem({
      status: 'processing',
      progress: 68.4,
    });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={vi.fn()}
        onRemove={onRemove}
        t={defaultT as any}
      />
    );

    const removeBtn = screen.getByRole('button', { name: 'Delete audio.mp3' });
    fireEvent.click(removeBtn);

    expect(confirmSpy).toHaveBeenCalledWith(
      'Stop audio.mp3 (68%)?',
      expect.objectContaining({
        title: 'batch.cancel_confirm_title',
        variant: 'warning',
        confirmLabel: 'batch.stop_transcription',
        cancelLabel: 'batch.continue_transcribing',
      })
    );
    expect(onRemove).not.toHaveBeenCalled();
  });

  it('prompts confirmation when removing a processing item and proceeds if confirmed', async () => {
    const onRemove = vi.fn();
    const confirmSpy = vi.spyOn(useDialogStore.getState(), 'confirm').mockResolvedValue(true);
    const item = createItem({
      status: 'processing',
      progress: 25,
    });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={vi.fn()}
        onRemove={onRemove}
        t={defaultT as any}
      />
    );

    const removeBtn = screen.getByRole('button', { name: 'Delete audio.mp3' });
    fireEvent.click(removeBtn);

    expect(confirmSpy).toHaveBeenCalled();
    await waitFor(() => {
      expect(onRemove).toHaveBeenCalledWith('item-1');
    });
  });

  it('calls onActivate when clicking the item', () => {
    const onActivate = vi.fn();
    const item = createItem({ status: 'complete' });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={onActivate}
        onRemove={vi.fn()}
        t={defaultT as any}
      />
    );

    fireEvent.click(screen.getByRole('listitem'));
    expect(onActivate).toHaveBeenCalledWith('item-1');
  });

  it('calls onRetry when clicking retry button on failed item', () => {
    const onRetry = vi.fn();
    const item = createItem({ status: 'error', errorMessage: 'Network timeout' });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={vi.fn()}
        onRemove={vi.fn()}
        onRetry={onRetry}
        t={defaultT as any}
      />
    );
    const retryBtn = screen.getByRole('button', { name: 'batch.retry' });
    expect(retryBtn.getAttribute('data-tooltip')).toBe('batch.retry');
    expect(retryBtn.querySelector('.queue-item-retry-text')).toBeNull();
    fireEvent.click(retryBtn);
    expect(onRetry).toHaveBeenCalledWith('item-1');
  });
  it('renders transcode failed summary and project-unified tooltip for video format error', () => {
    const rawError =
      'Failed to decode audio file clip.mp4: built-in decoder cannot process this file and FFmpeg is not installed. Please install FFmpeg or use a supported format.';
    const item = createItem({
      status: 'error',
      filename: 'clip.mp4',
      errorMessage: rawError,
    });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={vi.fn()}
        onRemove={vi.fn()}
        t={defaultT as any}
      />
    );

    expect(screen.getByText('batch.transcode_failed')).toBeDefined();
    const errorEl = screen.getByText('batch.transcode_failed').closest('.queue-item-error');
    expect(errorEl?.getAttribute('data-tooltip')).toBe(rawError);
    expect(errorEl?.getAttribute('data-tooltip-pos')).toBe('bottom');
    expect(screen.getByRole('listitem').getAttribute('data-tooltip')).toBeNull();
    expect(screen.queryByText('batch.suggest_ffmpeg')).toBeNull();
  });
  it('renders raw error and project-unified tooltip when failed item has a decode/transcode error', () => {
    const item = createItem({
      status: 'error',
      filename: 'audio.wav',
      errorMessage: 'Audio decode error: corrupted stream',
    });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={vi.fn()}
        onRemove={vi.fn()}
        t={defaultT as any}
      />
    );

    expect(screen.getByText('Audio decode error: corrupted stream')).toBeDefined();
    const errorEl = screen
      .getByText('Audio decode error: corrupted stream')
      .closest('.queue-item-error');
    expect(errorEl?.getAttribute('data-tooltip')).toBe('Audio decode error: corrupted stream');
    expect(errorEl?.getAttribute('data-tooltip-pos')).toBe('bottom');
    expect(screen.getByRole('listitem').getAttribute('data-tooltip')).toBeNull();
    expect(screen.queryByText('batch.suggest_ffmpeg')).toBeNull();
  });
  it('renders raw error and project-unified tooltip when error is unrelated', () => {
    const item = createItem({
      status: 'error',
      filename: 'audio.wav',
      errorMessage: 'Network timeout',
    });

    render(
      <QueueItem
        item={item}
        isActive={false}
        onActivate={vi.fn()}
        onRemove={vi.fn()}
        t={defaultT as any}
      />
    );

    expect(screen.getByText('Network timeout')).toBeDefined();
    const errorEl = screen.getByText('Network timeout').closest('.queue-item-error');
    expect(errorEl?.getAttribute('data-tooltip')).toBe('Network timeout');
    expect(errorEl?.getAttribute('data-tooltip-pos')).toBe('bottom');
    expect(screen.getByRole('listitem').getAttribute('data-tooltip')).toBeNull();
    expect(screen.queryByText('batch.suggest_ffmpeg')).toBeNull();
  });
});
