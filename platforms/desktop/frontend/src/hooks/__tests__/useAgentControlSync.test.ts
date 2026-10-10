import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { useHistoryStore } from '../../stores/historyStore';
import { useTaskLedgerStore } from '../../stores/taskLedgerStore';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useAgentControlSync } from '../useAgentControlSync';

const listeners = new Map<string, (...args: unknown[]) => unknown>();
const unlistenMocks = new Map<string, Mock>();

vi.mock('../../services/tauri/platform/events', () => ({
  listen: vi.fn(async (eventName: string, callback: (...args: unknown[]) => unknown) => {
    listeners.set(eventName, callback);
    const unlisten = vi.fn(() => {
      listeners.delete(eventName);
    });
    unlistenMocks.set(eventName, unlisten);
    return unlisten;
  }),
}));

vi.mock('../../stores/historyStore', () => {
  const loadItems = vi.fn().mockResolvedValue(undefined);
  return {
    useHistoryStore: {
      getState: () => ({
        loadItems,
      }),
    },
  };
});

describe('useAgentControlSync', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
    unlistenMocks.clear();
    useTranscriptStore.setState({
      isAgentRecording: false,
      agentRecordingHistoryId: null,
    });
    useTaskLedgerStore.setState({
      tasks: [],
    });
  });

  it('updates isAgentRecording and reloads history when recordingStatus becomes active', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('agent-control-recording-status')).toBe(true);
    });

    await act(async () => {
      await listeners.get('agent-control-recording-status')?.({
        payload: { active: true, historyId: 'hist-agent-1' },
      });
    });

    expect(useTranscriptStore.getState().isAgentRecording).toBe(true);
    expect(useTranscriptStore.getState().agentRecordingHistoryId).toBe('hist-agent-1');
    expect(useHistoryStore.getState().loadItems).toHaveBeenCalled();
  });

  it('resets isAgentRecording and reloads history when recordingStatus becomes inactive', async () => {
    useTranscriptStore.setState({
      isAgentRecording: true,
      agentRecordingHistoryId: 'hist-agent-1',
    });

    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('agent-control-recording-status')).toBe(true);
    });

    await act(async () => {
      await listeners.get('agent-control-recording-status')?.({
        payload: { active: false },
      });
    });

    expect(useTranscriptStore.getState().isAgentRecording).toBe(false);
    expect(useTranscriptStore.getState().agentRecordingHistoryId).toBeNull();
    expect(useHistoryStore.getState().loadItems).toHaveBeenCalled();
  });

  it('reloads history when transcriptUpdated event is emitted', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('transcript-updated')).toBe(true);
    });

    await act(async () => {
      await listeners.get('transcript-updated')?.({
        payload: { historyId: 'hist-mcp-1' },
      });
    });

    expect(useHistoryStore.getState().loadItems).toHaveBeenCalled();
  });

  it('patches task progress in taskLedgerStore when batchProgress arrives for an active task', async () => {
    const patchTaskSpy = vi.spyOn(useTaskLedgerStore.getState(), 'patchTask');
    useTaskLedgerStore.setState({
      tasks: [
        {
          id: 'mcp-transcribe-1',
          kind: 'batchImport',
          status: 'running',
          title: 'file.mp3',
          progress: 0,
          createdAt: 1000,
          updatedAt: 1000,
          retryable: false,
          cancelable: true,
          recoverable: false,
        },
      ],
    });

    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('batch-progress')).toBe(true);
    });

    await act(async () => {
      await listeners.get('batch-progress')?.({
        payload: ['/path/to/file.mp3', 45, 'mcp-transcribe-1'],
      });
    });

    expect(patchTaskSpy).toHaveBeenCalledWith(
      'mcp-transcribe-1',
      { progress: 45 },
      { transient: true }
    );
  });

  it('unregisters all listeners on unmount', async () => {
    const { unmount } = renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.size).toBe(3);
    });

    unmount();

    for (const unlisten of unlistenMocks.values()) {
      expect(unlisten).toHaveBeenCalled();
    }
  });
});
