import { useState } from 'react';
import { useBoundSessionId } from '../contexts/TranscriptSessionContext';
import { getFeatureLlmConfig, isLlmConfigComplete } from '../services/llm/configUtils';
import { polishService } from '../services/polishService';
import { retranscribeService } from '../services/retranscribeService';
import { useDialogStore } from '../stores/dialogStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import { setSegmentsForSession } from '../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { resolveSidecarKey } from '../stores/transcriptSidecarState';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import type { TranscriptSegment } from '../types/transcript';

export function usePolishActions() {
  const boundSessionId = useBoundSessionId();
  const showError = useDialogStore((state) => state.showError);
  const config = useEffectiveConfigStore((state) => state.config);
  const segments = useTranscriptSessionStore((state) => state.segments);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const sidecarKey = resolveSidecarKey(boundSessionId, sourceHistoryId);

  const llmState = useTranscriptSidecarStore((state) => state.llmStates[sidecarKey]) || {
    isPolishing: false,
    polishProgress: 0,
    isRetranscribing: false,
    retranscribeProgress: 0,
  };
  const { isPolishing, polishProgress, isRetranscribing, retranscribeProgress } = llmState;
  const updateLlmState = useTranscriptSidecarStore((state) => state.updateLlmState);

  const [undoBySession, setUndoBySession] = useState<
    Record<string, TranscriptSegment[] | null | undefined>
  >({});
  const [redoBySession, setRedoBySession] = useState<
    Record<string, TranscriptSegment[] | null | undefined>
  >({});
  const undoSegments = undoBySession[boundSessionId] ?? null;
  const redoSegments = redoBySession[boundSessionId] ?? null;

  const setUndoSegments = (next: TranscriptSegment[] | null) => {
    setUndoBySession((prev) => ({ ...prev, [boundSessionId]: next }));
  };
  const setRedoSegments = (next: TranscriptSegment[] | null) => {
    setRedoBySession((prev) => ({ ...prev, [boundSessionId]: next }));
  };

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

    const targetSidecarKey = sidecarKey;
    updateLlmState({ isRetranscribing: true, retranscribeProgress: 0 }, targetSidecarKey);

    try {
      await retranscribeService.retranscribeCurrentRecord({
        sessionId: boundSessionId,
        historyId: sourceHistoryId,
        onProgress: (progress) => {
          updateLlmState({ retranscribeProgress: progress }, targetSidecarKey);
        },
      });

      // Clear old polish undo/redo states only for this session after successful re-transcription
      setUndoSegments(null);
      setRedoSegments(null);
    } catch (error) {
      await showError({
        code: 'polish.retranscribe_failed',
        messageKey: 'errors.polish.retranscribe_failed',
        cause: error,
      });
    } finally {
      updateLlmState({ isRetranscribing: false, retranscribeProgress: 0 }, targetSidecarKey);
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

    // Save current segments for undo scoped to the launching session
    setUndoSegments(JSON.parse(JSON.stringify(segments)));
    setRedoSegments(null);

    if (onPreStart) onPreStart();

    try {
      await polishService.polishTranscript({ sessionId: boundSessionId });
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

      setSegmentsForSession(boundSessionId, undoSegments);
      setUndoSegments(null);
      if (onSuccess) onSuccess();
    }
  };

  const handleRedoPolish = (onSuccess?: () => void) => {
    if (redoSegments) {
      // Save current state (original) back to undo
      setUndoSegments(JSON.parse(JSON.stringify(segments)));

      setSegmentsForSession(boundSessionId, redoSegments);
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
