import { useHistoryStore } from '../stores/historyStore';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { setTranscriptSegments } from '../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { isHistoryItemDraft } from '../types/history';
import { historyService } from './historyService';
import { transcriptionService } from './transcriptionService';
import { logger } from '../utils/logger';

class RetranscribeService {
    async retranscribeCurrentRecord(onProgress?: (progress: number) => void): Promise<void> {
        const historyId = useTranscriptSessionStore.getState().sourceHistoryId;

        if (!historyId || historyId === 'current') {
            throw new Error('No saved history record found. Please ensure the recording is saved.');
        }

        const items = await historyService.getAll();
        const item = items.find(i => i.id === historyId);

        if (!item || !item.audioPath) {
            throw new Error('History item or audio path not found.');
        }
        if (isHistoryItemDraft(item)) {
            throw new Error('Live recording draft must be completed before re-transcribing.');
        }

        const audioAbsolutePath = await historyService.getAudioAbsolutePath(item.audioPath);
        if (!audioAbsolutePath) {
            throw new Error('Audio file not found on disk.');
        }

        const config = getEffectiveConfigSnapshot();
        if (!config.offlineModelPath) {
            throw new Error('Batch import model not configured.');
        }

        // Configure transcription service for batch mode
        transcriptionService.setModelPath(config.offlineModelPath);
        transcriptionService.setEnableITN(config.enableITN ?? false);
        const language = config.language;

        // Perform transcription
        const segments = await transcriptionService.transcribeFile(
            audioAbsolutePath,
            onProgress,
            undefined, // We'll just collect the final array
            language === 'auto' ? undefined : language
        );

        // Update Store
        setTranscriptSegments(segments);

        // Save to History File
        await useHistoryStore.getState().updateTranscript(historyId, segments);
        logger.info(`[RetranscribeService] Successfully re-transcribed and saved history item: ${historyId}`);
    }
}

export const retranscribeService = new RetranscribeService();
