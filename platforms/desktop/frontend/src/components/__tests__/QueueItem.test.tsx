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

    const retryBtn = screen.getByRole('button', { name: 'common.retry' });
    fireEvent.click(retryBtn);
    expect(onRetry).toHaveBeenCalledWith('item-1');
  });
});
