import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from './historyService';
import { transcriptionService } from './transcriptionService';
import { logger } from '../utils/logger';
import { splitByPunctuation } from '../utils/segmentUtils';

class RetranscribeService {
    async retranscribeCurrentRecord(onProgress?: (progress: number) => void): Promise<void> {
        const store = useTranscriptStore.getState();
        const historyId = store.sourceHistoryId;

        if (!historyId || historyId === 'current') {
            throw new Error('No saved history record found. Please ensure the recording is saved.');
        }

        const items = await historyService.getAll();
        const item = items.find(i => i.id === historyId);

        if (!item || !item.audioPath) {
            throw new Error('History item or audio path not found.');
        }

        const audioAbsolutePath = await historyService.getAudioAbsolutePath(item.audioPath);
        if (!audioAbsolutePath) {
            throw new Error('Audio file not found on disk.');
        }

        const config = store.config;
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

        const enableTimeline = config.enableTimeline ?? false;
        const finalSegments = enableTimeline ? splitByPunctuation(segments) : segments;

        // Update Store
        store.setSegments(finalSegments);

        // Save to History File
        await historyService.updateTranscript(historyId, finalSegments);
        logger.info(`[RetranscribeService] Successfully re-transcribed and saved history item: ${historyId}`);
    }
}

export const retranscribeService = new RetranscribeService();
