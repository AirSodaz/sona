import { useEffect } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { transcriptionService } from '../services/transcriptionService';
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
            if (!config.offlineModelPath) {
                return;
            }

            try {
                transcriptionService.setModelPath(config.offlineModelPath);
                transcriptionService.setLanguage(config.language);
                transcriptionService.setEnableITN(config.enableITN ?? false);
                transcriptionService.setPunctuationModelPath(config.punctuationModelPath || '');
                transcriptionService.setVadModelPath(config.vadModelPath || '');
                transcriptionService.setVadBufferSize(config.vadBufferSize || 5);

                const enabledITNModels = new Set(config.enabledITNModels || []);
                const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];
                if (enabledITNModels.size > 0) {
                    const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                    transcriptionService.setITNModelPaths(paths);
                } else {
                    transcriptionService.setITNModelPaths([]);
                }

                await transcriptionService.prepare();
            } catch (err) {
                console.error('[useTranscriptionServiceSync] Failed to prepare transcription service:', err);
            }
        };

        syncAndPrepare();
    }, [
        config.offlineModelPath,
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
