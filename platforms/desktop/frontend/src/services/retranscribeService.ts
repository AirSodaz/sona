import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { setSegmentsForHistory } from '../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { isHistoryItemDraft } from '../types/history';
import { logger } from '../utils/logger';
import { getScenarioLanguage } from '../utils/scenarioModels';
import { isAsrRequestConfigured, resolveAsrTranscriptionRequest } from './asrConfigService';
import { historyService } from './historyService';
import {
  captureTaskTarget,
  resolveLatestSegmentsForTarget,
  resolveTargetHistoryId,
  runWithDocumentLock,
} from './task/taskTarget';
import { type TranscriptionService, transcriptionService } from './transcriptionService';
import { transcriptSnapshotService } from './transcriptSnapshotService';

export interface RetranscribeRecordOptions {
  onProgress?: (progress: number) => void;
  sessionId?: string | null;
  historyId?: string | null;
}

export interface RetranscribeServicePorts {
  getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
  getTranscriptSessionStore: typeof useTranscriptSessionStore.getState;
  getHistoryStore: typeof useHistoryStore.getState;
  setSegmentsForHistory: typeof setSegmentsForHistory;
  historyService: typeof historyService;
  transcriptSnapshotService: typeof transcriptSnapshotService;
  transcriptionService: TranscriptionService;
}

export class RetranscribeService {
  constructor(private readonly ports: RetranscribeServicePorts) {}

  async retranscribeCurrentRecord(
    onProgressOrOptions?: ((progress: number) => void) | RetranscribeRecordOptions
  ): Promise<void> {
    const options: RetranscribeRecordOptions =
      typeof onProgressOrOptions === 'function'
        ? { onProgress: onProgressOrOptions }
        : (onProgressOrOptions ?? {});

    const sessionId = options.sessionId ?? null;
    const sessionSnapshot = sessionId
      ? useTranscriptStore.getState().sessions[sessionId]
      : undefined;
    const fallbackSession = this.ports.getTranscriptSessionStore();
    const initialHistoryId =
      options.historyId ??
      (sessionSnapshot ? sessionSnapshot.sourceHistoryId : fallbackSession.sourceHistoryId);

    const target = captureTaskTarget({
      sessionId,
      historyId: initialHistoryId,
    });

    const resolvedHistoryId = resolveTargetHistoryId(target);
    if (!resolvedHistoryId) {
      throw new Error('No saved history record found. Please ensure the recording is saved.');
    }

    await runWithDocumentLock(target, { taskType: 'retranscribe' }, async () => {
      const historyId = resolveTargetHistoryId(target);
      if (!historyId) {
        throw new Error('No saved history record found. Please ensure the recording is saved.');
      }

      const items = await this.ports.historyService.getAll();
      const item = items.find((i) => i.id === historyId);

      if (!item?.audioPath) {
        throw new Error('History item or audio path not found.');
      }
      if (isHistoryItemDraft(item)) {
        throw new Error('Live recording draft must be completed before re-transcribing.');
      }

      const audioAbsolutePath = await this.ports.historyService.getAudioAbsolutePath(item.id);
      if (!audioAbsolutePath) {
        throw new Error('Audio file not found on disk.');
      }

      const config = this.ports.getEffectiveConfigSnapshot();
      const batchAsr = resolveAsrTranscriptionRequest(config, 'batch');
      if (!isAsrRequestConfigured(batchAsr)) {
        throw new Error('Batch ASR is not configured.');
      }

      // Configure transcription service for batch mode
      if (batchAsr.engine === 'local') {
        this.ports.transcriptionService.setModelPath(batchAsr.modelPath);
      }
      this.ports.transcriptionService.setEnableITN(config.enableITN ?? false);
      const language = getScenarioLanguage(config, 'batch');
      const currentSegments = await resolveLatestSegmentsForTarget(
        target,
        sessionSnapshot?.segments ?? fallbackSession.segments
      );

      await this.ports.transcriptSnapshotService.createSnapshot(
        historyId,
        'retranscribe',
        currentSegments
      );

      // Perform transcription
      const segments = await this.ports.transcriptionService.transcribeFile(
        audioAbsolutePath,
        options.onProgress,
        undefined, // We'll just collect the final array
        language === 'auto' ? undefined : language,
        undefined,
        undefined,
        undefined,
        item.projectId ?? null
      );

      const historyStore = this.ports.getHistoryStore();
      if (typeof historyStore.commitSystemTranscriptUpdate === 'function') {
        await historyStore.commitSystemTranscriptUpdate(historyId, segments);
      } else {
        this.ports.setSegmentsForHistory(historyId, segments);
        await historyStore.updateTranscript(historyId, segments);
      }
      logger.info(
        `[RetranscribeService] Successfully re-transcribed and saved history item: ${historyId}`
      );
    });
  }
}

export function createRetranscribeService(ports: RetranscribeServicePorts): RetranscribeService {
  return new RetranscribeService(ports);
}

export const retranscribeService = createRetranscribeService({
  getEffectiveConfigSnapshot,
  getTranscriptSessionStore: useTranscriptSessionStore.getState,
  getHistoryStore: useHistoryStore.getState,
  setSegmentsForHistory,
  historyService,
  transcriptSnapshotService,
  transcriptionService,
});
