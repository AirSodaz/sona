import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createReactI18nextMock } from '../../__tests__/testUtils/i18n';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { QueueGroupedList } from '../QueueGroupedList';

vi.mock('react-i18next', () => createReactI18nextMock());

vi.mock('../../services/tauri/invoke', () => ({
  invokeTauri: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: vi.fn((path) => `asset://${path}`),
}));

describe('QueueGroupedList', () => {
  beforeEach(() => {
    useBatchQueueStore.setState({
      queueItems: [],
      activeItemId: null,
      isQueueProcessing: false,
      isQueuePaused: false,
    });
  });

  it('renders null when queue is empty', () => {
    const { container } = render(<QueueGroupedList />);
    expect(container.firstChild).toBeNull();
  });

  it('partitions items into processing, waiting, completed, and failed groups', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'proc-1',
          filename: 'proc.wav',
          filePath: '/proc.wav',
          status: 'processing',
          progress: 45,
          segments: [],
          projectId: null,
        },
        {
          id: 'pend-1',
          filename: 'pend.wav',
          filePath: '/pend.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          projectId: null,
        },
        {
          id: 'comp-1',
          filename: 'comp.wav',
          filePath: '/comp.wav',
          status: 'complete',
          progress: 100,
          segments: [],
          projectId: null,
        },
        {
          id: 'fail-1',
          filename: 'fail.wav',
          filePath: '/fail.wav',
          status: 'error',
          progress: 10,
          errorMessage: 'Corrupt audio',
          segments: [],
          projectId: null,
        },
      ],
    });

    render(<QueueGroupedList />);

    // Group titles
    expect(screen.getByText('batch.group_processing')).toBeDefined();
    expect(screen.getByText('batch.group_pending')).toBeDefined();
    expect(screen.getByText('batch.group_complete')).toBeDefined();
    expect(screen.getByText('batch.group_failed')).toBeDefined();

    expect(screen.getByText('proc.wav')).toBeDefined();
    expect(screen.getByText('pend.wav')).toBeDefined();
    expect(screen.getByText('fail.wav')).toBeDefined();

    // Completed group is collapsed by default when there are active/pending items; expand to verify
    const completeHeader = screen.getByRole('button', { name: /batch\.group_complete/ });
    fireEvent.click(completeHeader);
    expect(screen.getByText('comp.wav')).toBeDefined();
  });

  it('allows expanding and collapsing a group', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'proc-1',
          filename: 'proc.wav',
          filePath: '/proc.wav',
          status: 'processing',
          progress: 45,
          segments: [],
          projectId: null,
        },
      ],
    });

    render(<QueueGroupedList />);
    expect(screen.getByText('proc.wav')).toBeDefined();

    // Click header to collapse
    const header = screen.getByRole('button', { name: /batch\.group_processing/ });
    fireEvent.click(header);

    // After collapsing, item is no longer visible
    expect(screen.queryByText('proc.wav')).toBeNull();

    // Click again to expand
    fireEvent.click(header);
    expect(screen.getByText('proc.wav')).toBeDefined();
  });

  it('triggers retryAllFailed when clicking retry all in failed group header', () => {
    const retryAllSpy = vi
      .spyOn(useBatchQueueStore.getState(), 'retryAllFailed')
      .mockImplementation(() => {});

    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'fail-1',
          filename: 'fail.wav',
          filePath: '/fail.wav',
          status: 'error',
          progress: 0,
          segments: [],
          projectId: null,
        },
      ],
    });

    render(<QueueGroupedList />);

    const retryAllBtn = screen.getByRole('button', { name: 'batch.retry_all' });
    fireEvent.click(retryAllBtn);
    expect(retryAllSpy).toHaveBeenCalled();
  });
});
