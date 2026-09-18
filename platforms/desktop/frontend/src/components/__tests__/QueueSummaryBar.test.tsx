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

  it('allows clearing completed items or all items via the clear menu', () => {
    const clearAllSpy = vi.spyOn(useBatchQueueStore.getState(), 'clearQueue');
    const clearCompletedSpy = vi.spyOn(useBatchQueueStore.getState(), 'clearCompleted');
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
          status: 'pending',
          progress: 0,
          segments: [],
          projectId: null,
        },
      ],
    });

    const { rerender } = render(<QueueSummaryBar onAddFiles={vi.fn()} />);
    const trigger = screen.getByTestId('queue-clear-trigger');
    fireEvent.click(trigger);

    // Both options visible
    const clearCompletedBtn = screen.getByRole('menuitem', { name: /batch\.clear_completed/ });
    expect(screen.getByRole('menuitem', { name: /batch\.clear_all/ })).toBeDefined();

    fireEvent.click(clearCompletedBtn);
    expect(clearCompletedSpy).toHaveBeenCalled();

    // Re-open and click clear all
    rerender(<QueueSummaryBar onAddFiles={vi.fn()} />);
    fireEvent.click(trigger);
    const reopenedClearAllBtn = screen.getByRole('menuitem', { name: /batch\.clear_all/ });
    fireEvent.click(reopenedClearAllBtn);
    expect(clearAllSpy).toHaveBeenCalled();
  });

  it('does not render start button when there are no pending items', () => {
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
          status: 'error',
          progress: 0,
          segments: [],
          projectId: null,
        },
      ],
      isQueueProcessing: false,
      isQueuePaused: false,
    });

    render(<QueueSummaryBar onAddFiles={vi.fn()} />);
    expect(screen.queryByRole('button', { name: 'batch.start_queue' })).toBeNull();
  });

  it('renders start button and starts queue when pending items exist', () => {
    const processSpy = vi
      .spyOn(useBatchQueueStore.getState(), 'processQueue')
      .mockImplementation(async () => {});

    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.wav',
          filePath: '/file1.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          projectId: null,
        },
      ],
      isQueueProcessing: false,
      isQueuePaused: false,
    });

    render(<QueueSummaryBar onAddFiles={vi.fn()} />);
    const startBtn = screen.getByRole('button', { name: 'batch.start_queue' });
    fireEvent.click(startBtn);
    expect(processSpy).toHaveBeenCalled();
  });

  it('renders fill-error with 100% width when all items fail', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.wav',
          filePath: '/file1.wav',
          status: 'error',
          progress: 0,
          segments: [],
          projectId: null,
        },
      ],
      isQueueProcessing: false,
      isQueuePaused: false,
    });

    const { container } = render(<QueueSummaryBar onAddFiles={vi.fn()} />);
    const fill = container.querySelector('.queue-summary-progress-fill') as HTMLElement;
    expect(fill).toBeDefined();
    expect(fill.classList.contains('fill-error')).toBe(true);
    expect(fill.style.width).toBe('100%');
  });
});
