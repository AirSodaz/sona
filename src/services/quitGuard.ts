import i18n from '../i18n';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useDialogStore } from '../stores/dialogStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { logger } from '../utils/logger';
import {
  forceExit,
  hasActiveDownloads,
} from './tauri/app';

type TranscriptQuitTaskSnapshot = Pick<
  ReturnType<typeof useTranscriptRuntimeStore.getState>,
  'isRecording' | 'isPaused' | 'isCaptionMode' | 'processingStatus'
> & Pick<
  ReturnType<typeof useTranscriptSidecarStore.getState>,
  'llmStates' | 'summaryStates'
>;

type BatchQueueQuitTaskSnapshot = Pick<
  ReturnType<typeof useBatchQueueStore.getState>,
  'queueItems' | 'isQueueProcessing'
>;

function hasActiveLlmTasks(state: TranscriptQuitTaskSnapshot): boolean {
  return Object.values(state.llmStates).some((llmState) => (
    llmState.isTranslating
    || llmState.isPolishing
    || llmState.isRetranscribing
  ));
}

function hasActiveSummaryTasks(state: TranscriptQuitTaskSnapshot): boolean {
  return Object.values(state.summaryStates).some((summaryState) => summaryState.isGenerating);
}

function hasActiveBatchQueueTasks(state: BatchQueueQuitTaskSnapshot): boolean {
  const hasProcessingItem = state.queueItems.some((item) => item.status === 'processing');
  const hasPendingItemWhileQueueRunning = state.isQueueProcessing
    && state.queueItems.some((item) => item.status === 'pending');

  return hasProcessingItem || hasPendingItemWhileQueueRunning;
}

export function hasActiveFrontendQuitTasks(
  transcriptState: TranscriptQuitTaskSnapshot,
  batchQueueState: BatchQueueQuitTaskSnapshot,
): boolean {
  return (
    transcriptState.isRecording
    || transcriptState.isPaused
    || transcriptState.isCaptionMode
    || transcriptState.processingStatus === 'processing'
    || hasActiveLlmTasks(transcriptState)
    || hasActiveSummaryTasks(transcriptState)
    || hasActiveBatchQueueTasks(batchQueueState)
  );
}

export async function shouldWarnBeforeQuit(
  transcriptState: TranscriptQuitTaskSnapshot = {
    ...useTranscriptRuntimeStore.getState(),
    llmStates: useTranscriptSidecarStore.getState().llmStates,
    summaryStates: useTranscriptSidecarStore.getState().summaryStates,
  },
  batchQueueState: BatchQueueQuitTaskSnapshot = useBatchQueueStore.getState(),
): Promise<boolean> {
  if (hasActiveFrontendQuitTasks(transcriptState, batchQueueState)) {
    return true;
  }

  try {
    return await hasActiveDownloads();
  } catch (error) {
    logger.error('Failed to check downloads before quit:', error);
    return false;
  }
}

export async function runGuardedQuit(onExit: () => Promise<void>): Promise<boolean> {
  const shouldWarn = await shouldWarnBeforeQuit();

  if (shouldWarn) {
    const confirmed = await useDialogStore.getState().confirm(
      i18n.t('tray.quit_warning_message'),
      {
        title: i18n.t('tray.quit_warning_title'),
        variant: 'warning',
        confirmLabel: i18n.t('tray.quit_confirm'),
        cancelLabel: i18n.t('common.cancel'),
      },
    );

    if (!confirmed) {
      return false;
    }
  }

  await onExit();
  return true;
}

export async function forceExitWithGuard(): Promise<boolean> {
  return runGuardedQuit(async () => {
    await forceExit();
  });
}
