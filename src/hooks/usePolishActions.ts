import { useState } from 'react';
import { useDialogStore } from '../stores/dialogStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { setTranscriptSegments } from '../stores/transcriptCoordinator';
import { polishService } from '../services/polishService';
import { retranscribeService } from '../services/retranscribeService';
import { TranscriptSegment } from '../types/transcript';
import { getFeatureLlmConfig, isLlmConfigComplete } from '../services/llm/configUtils';

export function usePolishActions() {
    const showError = useDialogStore((state) => state.showError);
    const config = useEffectiveConfigStore((state) => state.config);
    const segments = useTranscriptSessionStore((state) => state.segments);
    const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);

    const llmState = useTranscriptSidecarStore((state) => state.llmStates[sourceHistoryId || 'current']) || { isPolishing: false, polishProgress: 0, isRetranscribing: false, retranscribeProgress: 0 };
    const { isPolishing, polishProgress, isRetranscribing, retranscribeProgress } = llmState;
    const updateLlmState = useTranscriptSidecarStore((state) => state.updateLlmState);

    const [undoSegments, setUndoSegments] = useState<TranscriptSegment[] | null>(null);
    const [redoSegments, setRedoSegments] = useState<TranscriptSegment[] | null>(null);

    const handleRetranscribe = async (onPreStart?: () => void) => {
        if (isRetranscribing) return;

        if (!config.batchModelPath) {
            await showError({
                code: 'config.batch_model_missing',
                messageKey: 'errors.config.batch_model_missing',
                showCause: false,
            });
            return;
        }

        if (onPreStart) onPreStart();

        updateLlmState({ isRetranscribing: true, retranscribeProgress: 0 });

        try {
            await retranscribeService.retranscribeCurrentRecord((progress) => {
                updateLlmState({ retranscribeProgress: progress });
            });

            // Clear old polish undo/redo states only after successful re-transcription
            setUndoSegments(null);
            setRedoSegments(null);
        } catch (error) {
            await showError({
                code: 'polish.retranscribe_failed',
                messageKey: 'errors.polish.retranscribe_failed',
                cause: error,
            });
        } finally {
            updateLlmState({ isRetranscribing: false, retranscribeProgress: 0 });
        }
    };

    const handleStartPolish = async (onPreStart?: () => void) => {
        if (isPolishing) return;

        const llm = getFeatureLlmConfig(config, 'polish');
        if (!isLlmConfigComplete(llm)) {
            await showError({
                code: 'config.polish_model_missing',
                messageKey: 'errors.config.polish_model_missing',
                showCause: false,
            });
            return;
        }

        // Save current segments for undo
        setUndoSegments(JSON.parse(JSON.stringify(segments)));
        setRedoSegments(null);

        if (onPreStart) onPreStart();

        try {
            await polishService.polishTranscript();
        } catch (error) {
            await showError({
                code: 'polish.failed',
                messageKey: 'errors.polish.failed',
                cause: error,
            });
        }
    };

    const handleUndoPolish = (onSuccess?: () => void) => {
        if (undoSegments) {
            // Save current state for redo
            setRedoSegments(JSON.parse(JSON.stringify(segments)));

            setTranscriptSegments(undoSegments);
            setUndoSegments(null);
            if (onSuccess) onSuccess();
        }
    };

    const handleRedoPolish = (onSuccess?: () => void) => {
        if (redoSegments) {
            // Save current state (original) back to undo
            setUndoSegments(JSON.parse(JSON.stringify(segments)));

            setTranscriptSegments(redoSegments);
            setRedoSegments(null);
            if (onSuccess) onSuccess();
        }
    };

    return {
        isPolishing,
        polishProgress,
        isRetranscribing,
        retranscribeProgress,
        undoSegments,
        redoSegments,
        handleStartPolish,
        handleRetranscribe,
        handleUndoPolish,
        handleRedoPolish,
    };
}
