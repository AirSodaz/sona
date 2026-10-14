import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { historyService } from '../../services/historyService';
import { APP_SETTING_UPDATED_EVENT, STORE_KEY_CONFIG } from '../../services/storageService';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { useHistoryStore } from '../../stores/historyStore';
import { useTaskLedgerStore } from '../../stores/taskLedgerStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { sessionActions } from '../../stores/transcriptSessionStore';
import { useTranscriptStore } from '../../stores/transcriptStore';
import type { TranscriptSegment } from '../../types/transcript';
import { useAgentControlSync } from '../useAgentControlSync';

const stopActiveRecordingMock = vi.fn().mockResolvedValue(true);
vi.mock('../../stores/transcriptCoordinator', () => ({
  stopActiveRecording: (options?: { discard?: boolean }) => stopActiveRecordingMock(options),
}));

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

vi.mock('../../services/historyService', () => ({
  historyService: {
    loadTranscript: vi.fn(),
  },
}));

describe('useAgentControlSync', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
    unlistenMocks.clear();
    useTranscriptRuntimeStore.setState({
      isRecording: false,
      isPaused: false,
    });
    useTranscriptStore.setState({
      isRecording: false,
      isPaused: false,
      isAgentRecording: false,
      agentRecordingHistoryId: null,
    });
    useTaskLedgerStore.setState({
      tasks: [],
    });
    useConfigStore.setState({
      config: DEFAULT_CONFIG,
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
    expect(useTranscriptRuntimeStore.getState().isRecording).toBe(true);
    expect(useTranscriptRuntimeStore.getState().isPaused).toBe(false);
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
    expect(useTranscriptRuntimeStore.getState().isRecording).toBe(false);
    expect(useTranscriptRuntimeStore.getState().isPaused).toBe(false);
    expect(useHistoryStore.getState().loadItems).toHaveBeenCalled();
  });

  it('calls stopActiveRecording with discard false when agent requests UI recording stop', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('agent-control-ui-recording-stop-requested')).toBe(true);
    });

    await act(async () => {
      await listeners.get('agent-control-ui-recording-stop-requested')?.({
        payload: { historyId: 'hist-ui-1', discard: false },
      });
    });

    expect(stopActiveRecordingMock).toHaveBeenCalledWith({ discard: false });
  });

  it('calls stopActiveRecording with discard true when agent requests UI recording discard', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('agent-control-ui-recording-stop-requested')).toBe(true);
    });

    await act(async () => {
      await listeners.get('agent-control-ui-recording-stop-requested')?.({
        payload: { historyId: 'hist-ui-2', discard: true },
      });
    });

    expect(stopActiveRecordingMock).toHaveBeenCalledWith({ discard: true });
  });

  it('calls stopActiveRecording when recordingStatus reports active false during active UI recording', async () => {
    useTranscriptStore.setState({
      isRecording: true,
      isAgentRecording: false,
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

    expect(stopActiveRecordingMock).toHaveBeenCalled();
  });

  it('updates isPaused in runtime store when recordingStatus reports paused', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('agent-control-recording-status')).toBe(true);
    });

    await act(async () => {
      await listeners.get('agent-control-recording-status')?.({
        payload: { active: true, isPaused: true, historyId: 'hist-agent-1' },
      });
    });

    expect(useTranscriptRuntimeStore.getState().isRecording).toBe(true);
    expect(useTranscriptRuntimeStore.getState().isPaused).toBe(true);
    expect(useTranscriptStore.getState().isPaused).toBe(true);
  });

  it('reloads history and reloads active transcript segments if sourceHistoryId matches', async () => {
    const mockSegments: TranscriptSegment[] = [
      { id: 'seg-1', text: 'Updated segment text', start: 0, end: 1, isFinal: true },
    ];
    vi.mocked(historyService.loadTranscript).mockResolvedValue(mockSegments);
    const setSegmentsSpy = vi.spyOn(sessionActions, 'setSegments');

    useTranscriptStore.getState().setSourceHistoryId('hist-active-1');

    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('transcript-updated')).toBe(true);
    });

    await act(async () => {
      await listeners.get('transcript-updated')?.({
        payload: { historyId: 'hist-active-1' },
      });
    });

    expect(useHistoryStore.getState().loadItems).toHaveBeenCalled();
    expect(historyService.loadTranscript).toHaveBeenCalledWith('hist-active-1');
    expect(setSegmentsSpy).toHaveBeenCalledWith(mockSegments);
  });

  it('reloads history but does not reload transcript segments if sourceHistoryId does not match', async () => {
    vi.mocked(historyService.loadTranscript).mockResolvedValue([]);
    const setSegmentsSpy = vi.spyOn(sessionActions, 'setSegments');

    useTranscriptStore.getState().setSourceHistoryId('hist-other');

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
    expect(historyService.loadTranscript).not.toHaveBeenCalled();
    expect(setSegmentsSpy).not.toHaveBeenCalled();
  });

  it('does not set segments if active session changes while loadTranscript is in flight', async () => {
    let resolveTranscript: (value: TranscriptSegment[]) => void = () => {};
    const pendingPromise = new Promise<TranscriptSegment[]>((resolve) => {
      resolveTranscript = resolve;
    });
    vi.mocked(historyService.loadTranscript).mockReturnValue(pendingPromise);
    const setSegmentsSpy = vi.spyOn(sessionActions, 'setSegments');

    useTranscriptStore.getState().setSourceHistoryId('hist-active-1');

    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('transcript-updated')).toBe(true);
    });

    let handlerPromise: Promise<unknown> | undefined;
    act(() => {
      handlerPromise = listeners.get('transcript-updated')?.({
        payload: { historyId: 'hist-active-1' },
      }) as Promise<unknown>;
    });

    // User navigates to another history item before loadTranscript resolves
    useTranscriptStore.getState().setSourceHistoryId('hist-navigated-away');

    await act(async () => {
      resolveTranscript([{ id: 'seg-1', text: 'Late arrival', start: 0, end: 1, isFinal: true }]);
      await handlerPromise;
    });

    expect(historyService.loadTranscript).toHaveBeenCalledWith('hist-active-1');
    expect(setSegmentsSpy).not.toHaveBeenCalled();
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

  it('subscribes to mcp statusChanged and activity events', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has('mcp-status-changed')).toBe(true);
      expect(listeners.has('mcp-activity')).toBe(true);
      expect(listeners.has('mcp-mutation-notice')).toBe(true);
    });
  });

  it('updates useConfigStore when app-setting-updated event is received with single key-value', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has(APP_SETTING_UPDATED_EVENT)).toBe(true);
    });

    await act(async () => {
      await listeners.get(APP_SETTING_UPDATED_EVENT)?.({
        payload: { key: 'theme', value: 'dark' },
      });
    });

    expect(useConfigStore.getState().config.theme).toBe('dark');
  });

  it('updates useConfigStore when app-setting-updated event is received with STORE_KEY_CONFIG', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has(APP_SETTING_UPDATED_EVENT)).toBe(true);
    });

    await act(async () => {
      await listeners.get(APP_SETTING_UPDATED_EVENT)?.({
        payload: { key: STORE_KEY_CONFIG, value: { theme: 'light', appLanguage: 'fr' } },
      });
    });

    expect(useConfigStore.getState().config.theme).toBe('light');
    expect(useConfigStore.getState().config.appLanguage).toBe('fr');
  });

  it('ignores non-config keys such as sona- prefixed storage keys', async () => {
    renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.has(APP_SETTING_UPDATED_EVENT)).toBe(true);
    });

    await act(async () => {
      await listeners.get(APP_SETTING_UPDATED_EVENT)?.({
        payload: { key: 'sona-active-project-id', value: 'proj-123' },
      });
    });

    const config = useConfigStore.getState().config as Record<string, unknown>;
    expect(config['sona-active-project-id']).toBeUndefined();
  });

  it('unregisters all listeners on unmount', async () => {
    const { unmount } = renderHook(() => useAgentControlSync());

    await waitFor(() => {
      expect(listeners.size).toBe(8);
    });

    unmount();

    for (const unlisten of unlistenMocks.values()) {
      expect(unlisten).toHaveBeenCalled();
    }
  });
});
