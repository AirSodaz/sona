import { useEffect } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore } from '../stores/configStore';
import { transcriptionService, captionTranscriptionService } from '../services/transcriptionService';
import { logger } from '../utils/logger';

export function useTranscriptionServiceSync() {
    const config = useConfigStore((state) => state.config);
    const isRecording = useTranscriptStore((state) => state.isRecording);

    useEffect(() => {
        // We only want to auto-prepare if we are NOT actively recording,
        // because we don't want to restart the recognizer and interrupt a live recording.
        if (isRecording) {
            return;
        }

        const syncAndPrepare = async () => {
            if (!config.streamingModelPath) {
                return;
            }

            try {
                transcriptionService.setModelPath(config.streamingModelPath);
                transcriptionService.setLanguage(config.language);
                transcriptionService.setEnableITN(config.enableITN ?? false);

                captionTranscriptionService.setModelPath(config.streamingModelPath);
                captionTranscriptionService.setLanguage(config.language);
                captionTranscriptionService.setEnableITN(config.enableITN ?? false);

                await transcriptionService.prepare();
                await captionTranscriptionService.prepare();
            } catch (err) {
                logger.error('[useTranscriptionServiceSync] Failed to prepare transcription service:', err);
            }
        };

        syncAndPrepare();
    }, [
        config.streamingModelPath,
        config.language,
        config.enableITN,
        config.punctuationModelPath,
        config.vadModelPath,
        config.vadBufferSize,
        isRecording
    ]);
}
