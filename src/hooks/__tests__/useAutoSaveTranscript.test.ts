import { act, renderHook } from '@testing-library/react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { useAutoSaveTranscript } from '../useAutoSaveTranscript';
import { useHistoryStore } from '../../stores/historyStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';

vi.mock('../../services/historyService', () => ({
  historyService: {
    updateTranscript: vi.fn(),
    updateItemMeta: vi.fn(),
  },
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    info: vi.fn(),
    error: vi.fn(),
  },
}));

describe('useAutoSaveTranscript', () => {
  const flushMicrotasks = async () => {
    await act(async () => {
      await Promise.resolve();
    });
  };

  beforeEach(async () => {
    vi.clearAllMocks();
    vi.useFakeTimers();

    useHistoryStore.setState({
      items: [],
      isLoading: false,
      error: null,
    } as any);

    useTranscriptStore.setState({
      segments: [],
      sourceHistoryId: null,
      autoSaveStates: {},
    });

    const { historyService } = await import('../../services/historyService');
    (historyService.updateTranscript as any).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('transitions from saving to saved for persisted transcript edits', async () => {
    useTranscriptStore.setState({
      sourceHistoryId: 'hist-1',
      segments: [{ id: 'seg-1', text: 'Hello', start: 0, end: 1, isFinal: true }],
    });

    renderHook(() => useAutoSaveTranscript());

    act(() => {
      useTranscriptStore.getState().updateSegment('seg-1', { text: 'Hello world' });
    });

    expect(useTranscriptStore.getState().autoSaveStates['hist-1']?.status).toBe('saving');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    await flushMicrotasks();

    const { historyService } = await import('../../services/historyService');
    expect(historyService.updateTranscript).toHaveBeenCalledWith('hist-1', expect.any(Array));
    expect(useTranscriptStore.getState().autoSaveStates['hist-1']?.status).toBe('saved');
  });

  it('sets error status when transcript persistence fails', async () => {
    const { historyService } = await import('../../services/historyService');
    (historyService.updateTranscript as any).mockRejectedValueOnce(new Error('Disk full'));

    useTranscriptStore.setState({
      sourceHistoryId: 'hist-1',
      segments: [{ id: 'seg-1', text: 'Hello', start: 0, end: 1, isFinal: true }],
    });

    renderHook(() => useAutoSaveTranscript());

    act(() => {
      useTranscriptStore.getState().updateSegment('seg-1', { text: 'Hello again' });
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    await flushMicrotasks();

    expect(useTranscriptStore.getState().autoSaveStates['hist-1']?.status).toBe('error');
  });

  it('flushes the previous item on switch and does not leak status to the next item', async () => {
    useTranscriptStore.setState({
      sourceHistoryId: 'hist-1',
      segments: [{ id: 'seg-1', text: 'One', start: 0, end: 1, isFinal: true }],
    });

    renderHook(() => useAutoSaveTranscript());

    act(() => {
      useTranscriptStore.getState().updateSegment('seg-1', { text: 'One updated' });
    });

    expect(useTranscriptStore.getState().autoSaveStates['hist-1']?.status).toBe('saving');

    act(() => {
      useTranscriptStore.getState().loadTranscript(
        [{ id: 'seg-2', text: 'Two', start: 0, end: 1, isFinal: true }],
        'hist-2',
      );
    });
    await flushMicrotasks();

    const { historyService } = await import('../../services/historyService');
    expect(historyService.updateTranscript).toHaveBeenCalledWith('hist-1', expect.any(Array));
    expect(useTranscriptStore.getState().autoSaveStates['hist-1']?.status).toBe('saved');

    expect(useTranscriptStore.getState().autoSaveStates['hist-2']).toBeUndefined();
  });
});
