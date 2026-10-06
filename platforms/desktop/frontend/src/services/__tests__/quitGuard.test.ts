import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as configPersistence from '../../hooks/useConfigPersistence';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import { hasActiveFrontendQuitTasks, runGuardedQuit } from '../quitGuard';
import { transcriptAutoSaveRuntime } from '../transcriptAutoSaveRuntime';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string) => key,
  },
}));

function resetQuitGuardStores() {
  useTranscriptStore.setState({
    isRecording: false,
    isPaused: false,
    isCaptionMode: false,
    processingStatus: 'idle',
    llmStates: {},
    summaryStates: {},
  });
  useBatchQueueStore.setState({
    queueItems: [],
    activeItemId: null,
    isQueueProcessing: false,
  });
}

describe('quitGuard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetQuitGuardStores();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'has_active_downloads') {
        return false;
      }

      throw new Error(`Unexpected invoke command: ${command}`);
    });
  });

  it('returns false when no quit-blocking tasks are active', () => {
    expect(
      hasActiveFrontendQuitTasks(useTranscriptStore.getState(), useBatchQueueStore.getState())
    ).toBe(false);
  });

  it.each([
    ['paused recording', { isPaused: true }, {}],
    ['live caption', { isCaptionMode: true }, {}],
    ['compat processing state', { processingStatus: 'processing' }, {}],
    ['translation', { llmStates: { current: { isTranslating: true } } }, {}],
    ['polish', { llmStates: { current: { isPolishing: true } } }, {}],
    ['retranscribe', { llmStates: { current: { isRetranscribing: true } } }, {}],
    ['summary generation', { summaryStates: { current: { isGenerating: true } } }, {}],
    ['processing batch queue item', {}, { queueItems: [{ status: 'processing' }] }],
    [
      'running queue with pending item',
      {},
      { isQueueProcessing: true, queueItems: [{ status: 'pending' }] },
    ],
  ])('detects %s as an active quit task', (_label, transcriptPatch, batchPatch) => {
    useTranscriptStore.setState(
      transcriptPatch as Partial<ReturnType<typeof useTranscriptStore.getState>>
    );
    useBatchQueueStore.setState(
      batchPatch as Partial<ReturnType<typeof useBatchQueueStore.getState>>
    );

    expect(
      hasActiveFrontendQuitTasks(useTranscriptStore.getState(), useBatchQueueStore.getState())
    ).toBe(true);
  });

  it('exits immediately without prompting when no active tasks are found', async () => {
    const confirmMock = vi.fn();
    const exitMock = vi.fn().mockResolvedValue(undefined);
    useDialogStore.setState({ confirm: confirmMock as any });

    const result = await runGuardedQuit(exitMock);

    expect(result).toBe(true);
    expect(invoke).toHaveBeenCalledWith('has_active_downloads');
    expect(confirmMock).not.toHaveBeenCalled();
    expect(exitMock).toHaveBeenCalledTimes(1);
  });

  it('prompts and exits when a quit-blocking task is active and the user confirms', async () => {
    const confirmMock = vi.fn().mockResolvedValue(true);
    const exitMock = vi.fn().mockResolvedValue(undefined);
    useTranscriptStore.setState({
      summaryStates: {
        current: {
          activeTemplateId: 'general',
          isGenerating: true,
          generationProgress: 0,
        },
      },
    });
    useDialogStore.setState({ confirm: confirmMock as any });

    const result = await runGuardedQuit(exitMock);

    expect(result).toBe(true);
    expect(confirmMock).toHaveBeenCalledWith(
      'tray.quit_warning_message',
      expect.objectContaining({
        title: 'tray.quit_warning_title',
        confirmLabel: 'tray.quit_confirm',
        cancelLabel: 'common.cancel',
      })
    );
    expect(exitMock).toHaveBeenCalledTimes(1);
  });

  it('does not exit when the user cancels the quit confirmation', async () => {
    const confirmMock = vi.fn().mockResolvedValue(false);
    const exitMock = vi.fn().mockResolvedValue(undefined);
    useBatchQueueStore.setState({
      isQueueProcessing: true,
      queueItems: [{ status: 'pending' }] as any,
    });
    useDialogStore.setState({ confirm: confirmMock as any });

    const result = await runGuardedQuit(exitMock);

    expect(result).toBe(false);
    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(exitMock).not.toHaveBeenCalled();
  });

  it('prompts for active downloads even when frontend task state is idle', async () => {
    const confirmMock = vi.fn().mockResolvedValue(true);
    const exitMock = vi.fn().mockResolvedValue(undefined);
    useDialogStore.setState({ confirm: confirmMock as any });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'has_active_downloads') {
        return true;
      }

      throw new Error(`Unexpected invoke command: ${command}`);
    });

    const result = await runGuardedQuit(exitMock);

    expect(result).toBe(true);
    expect(confirmMock).toHaveBeenCalledTimes(1);
    expect(exitMock).toHaveBeenCalledTimes(1);
  });

  it('flushes pending transcript and config persistence before executing onExit', async () => {
    const exitMock = vi.fn().mockResolvedValue(undefined);
    const callOrder: string[] = [];

    const flushAutoSaveSpy = vi
      .spyOn(transcriptAutoSaveRuntime, 'flushPending')
      .mockImplementation(async () => {
        callOrder.push('flushAutoSave');
      });
    const flushConfigSpy = vi
      .spyOn(configPersistence, 'flushConfigPersistence')
      .mockImplementation(async () => {
        callOrder.push('flushConfig');
      });
    exitMock.mockImplementation(async () => {
      callOrder.push('exit');
    });

    const result = await runGuardedQuit(exitMock);

    expect(result).toBe(true);
    expect(flushAutoSaveSpy).toHaveBeenCalledTimes(1);
    expect(flushConfigSpy).toHaveBeenCalledTimes(1);
    expect(callOrder).toHaveLength(3);
    expect(callOrder[2]).toBe('exit');
  });

  it('still proceeds with onExit even if flush operations reject', async () => {
    const exitMock = vi.fn().mockResolvedValue(undefined);
    vi.spyOn(transcriptAutoSaveRuntime, 'flushPending').mockRejectedValue(
      new Error('autosave failure')
    );
    vi.spyOn(configPersistence, 'flushConfigPersistence').mockRejectedValue(
      new Error('config persistence failure')
    );

    const result = await runGuardedQuit(exitMock);

    expect(result).toBe(true);
    expect(exitMock).toHaveBeenCalledTimes(1);
  });
});
