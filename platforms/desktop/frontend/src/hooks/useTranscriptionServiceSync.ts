import { useEffect } from 'react';
import { useConfigStore } from '../stores/configStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { transcriptionService, captionTranscriptionService } from '../services/transcriptionService';
import { isAsrRequestConfigured, resolveAsrTranscriptionRequest } from '../services/asrConfigService';
import { logger } from '../utils/logger';

export function useTranscriptionServiceSync() {
    const config = useConfigStore((state) => state.config);
    const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);

    useEffect(() => {
        // We only want to auto-prepare if we are NOT actively recording,
        // because we don't want to restart the recognizer and interrupt a live recording.
        if (isRecording) {
            return;
        }

        const syncAndPrepare = async () => {
            const liveAsr = resolveAsrTranscriptionRequest(config, 'live');
            const captionAsr = resolveAsrTranscriptionRequest(config, 'caption');
            if (!isAsrRequestConfigured(liveAsr) && !isAsrRequestConfigured(captionAsr)) {
                return;
            }

            try {
                if (liveAsr.engine === 'local-sherpa') {
                    transcriptionService.setModelPath(liveAsr.modelPath);
                }
                transcriptionService.setLanguage(config.language);
                transcriptionService.setEnableITN(config.enableITN ?? false);

                if (captionAsr.engine === 'local-sherpa') {
                    captionTranscriptionService.setModelPath(captionAsr.modelPath);
                }
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
        config,
        isRecording
    ]);
}
