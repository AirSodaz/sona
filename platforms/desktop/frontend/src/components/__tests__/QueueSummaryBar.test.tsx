import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createReactI18nextMock } from '../../__tests__/testUtils/i18n';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { QueueSummaryBar } from '../QueueSummaryBar';

vi.mock('react-i18next', () => createReactI18nextMock());

vi.mock('../../services/tauri/invoke', () => ({
  invokeTauri: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: vi.fn((path) => `asset://${path}`),
}));

describe('QueueSummaryBar', () => {
  beforeEach(() => {
    useBatchQueueStore.setState({
      queueItems: [],
      activeItemId: null,
      isQueueProcessing: false,
      isQueuePaused: false,
    });
  });

  it('renders null when queue is empty', () => {
    const { container } = render(<QueueSummaryBar onAddFiles={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders overall queue count, status pill, and progress percentage', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.wav',
          filePath: '/file1.wav',
          status: 'complete',
          progress: 100,
          segments: [],
          projectId: null,
        },
        {
          id: 'item-2',
          filename: 'file2.wav',
          filePath: '/file2.wav',
          status: 'processing',
          progress: 50,
          segments: [],
          projectId: null,
        },
      ],
      isQueueProcessing: true,
      isQueuePaused: false,
    });

    render(<QueueSummaryBar onAddFiles={vi.fn()} />);

    expect(screen.getByText('Queue (2)')).toBeDefined();
    // 100 + 50 = 150 out of 200 = 75%
    expect(screen.getByText('75%')).toBeDefined();
    const progress = screen.getByRole('progressbar');
    expect(progress.getAttribute('aria-valuenow')).toBe('75');
  });

  it('allows pausing and resuming processing', () => {
    const pauseSpy = vi.spyOn(useBatchQueueStore.getState(), 'pauseQueue');
    const resumeSpy = vi.spyOn(useBatchQueueStore.getState(), 'resumeQueue');

    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.wav',
          filePath: '/file1.wav',
          status: 'processing',
          progress: 20,
          segments: [],
          projectId: null,
        },
      ],
      isQueueProcessing: true,
      isQueuePaused: false,
    });

    const { rerender } = render(<QueueSummaryBar onAddFiles={vi.fn()} />);

    const pauseBtn = screen.getByRole('button', { name: 'batch.pause_queue' });
    fireEvent.click(pauseBtn);
    expect(pauseSpy).toHaveBeenCalled();

    // Now state updates to paused
    useBatchQueueStore.setState({
      isQueueProcessing: false,
      isQueuePaused: true,
    });
    rerender(<QueueSummaryBar onAddFiles={vi.fn()} />);

    const resumeBtn = screen.getByRole('button', { name: 'batch.resume_queue' });
    fireEvent.click(resumeBtn);
    expect(resumeSpy).toHaveBeenCalled();
  });

  it('triggers onAddFiles when clicking add more files', () => {
    const onAddFiles = vi.fn();
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.wav',
          filePath: '/file1.wav',
          status: 'complete',
          progress: 100,
          segments: [],
          projectId: null,
        },
      ],
    });

    render(<QueueSummaryBar onAddFiles={onAddFiles} />);
    const addBtn = screen.getByRole('button', { name: 'batch.add_more_files' });
    fireEvent.click(addBtn);
    expect(onAddFiles).toHaveBeenCalled();
  });

  it('triggers clearQueue when clicking clear button', () => {
    const clearSpy = vi.spyOn(useBatchQueueStore.getState(), 'clearQueue');
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.wav',
          filePath: '/file1.wav',
          status: 'complete',
          progress: 100,
          segments: [],
          projectId: null,
        },
      ],
    });

    render(<QueueSummaryBar onAddFiles={vi.fn()} />);
    const clearBtn = screen.getByLabelText('batch.clear_queue');
    fireEvent.click(clearBtn);
    expect(clearSpy).toHaveBeenCalled();
  });
});
