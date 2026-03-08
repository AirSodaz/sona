import { useEffect } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { transcriptionService, captionTranscriptionService } from '../services/transcriptionService';
import { modelService } from '../services/modelService';

export function useTranscriptionServiceSync() {
    const config = useTranscriptStore((state) => state.config);
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

                const enabledITNModels = new Set(config.enabledITNModels || []);
                const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];
                if (enabledITNModels.size > 0) {
                    const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                    transcriptionService.setITNModelPaths(paths);
                    captionTranscriptionService.setITNModelPaths(paths);
                } else {
                    transcriptionService.setITNModelPaths([]);
                    captionTranscriptionService.setITNModelPaths([]);
                }

                await transcriptionService.prepare();
                await captionTranscriptionService.prepare();
            } catch (err) {
                console.error('[useTranscriptionServiceSync] Failed to prepare transcription service:', err);
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
        config.enabledITNModels,
        config.itnRulesOrder,
        isRecording
    ]);
}
