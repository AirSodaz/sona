import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  isAsrRequestConfigured,
  resolveAsrTranscriptionRequest,
} from '../services/asrConfigService';
import type { LiveRecordingDraftHandle } from '../services/historyService';
import { historyService } from '../services/historyService';
import { pipelineExecutionEngine } from '../services/pipeline/pipelineExecutionEngine';
import { type EffectivePipelineSnapshot, resolveItemPipeline } from '../services/projectPipeline';
import {
  collectRecordingTranslations,
  mergeRecordingTranslations,
  type RecordingTranslationDrain,
  waitForRecordingSegmentTasks,
} from '../services/recordingPostProcessing';
import { speakerService } from '../services/speakerService';
import { summaryService } from '../services/summaryService';
import {
  createTaskTarget,
  resolveLatestSegmentsForTarget,
  runWithDocumentLock,
} from '../services/task/taskTarget';
import { audioDeleteRecordingFile } from '../services/tauri/audio';
import { convertManagedAudioFileSrc } from '../services/tauri/platform/assets';
import { writeFile } from '../services/tauri/platform/fs';
import { transcriptionService } from '../services/transcriptionService';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useProjectStore } from '../stores/projectStore';
import {
  clearTranscriptSegments,
  registerActiveRecorderStopHandler,
  setSegmentsForHistory,
  setTranscriptSegments,
} from '../stores/transcriptCoordinator';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { DEFAULT_SESSION_DATA, useTranscriptStore } from '../stores/transcriptStore';
import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { logger } from '../utils/logger';
import { getResumeOnboardingStep } from '../utils/onboarding';
import { createAudioRecorderCapture, getSupportedMimeType } from './audioRecorder/capture';
import { createRecordingPersistence } from './audioRecorder/persistence';
import { createRecordController } from './audioRecorder/recordController';
import { createRecordSessionController } from './audioRecorder/session';
import { createRecordTimingController } from './audioRecorder/timing';
import type {
  InputSource,
  RecordSegmentDeliveryMeta,
  RecordSessionPhase,
} from './audioRecorder/types';

export type {
  RecordSegmentDeliveryMeta,
  RecordSessionPhase,
} from './audioRecorder/types';

export { getSupportedMimeType };

interface UseAudioRecorderProps {
  inputSource: InputSource;
  onSegment: (update: TranscriptUpdate, meta: RecordSegmentDeliveryMeta) => void;
}

interface RecordingAutomationSnapshot {
  config: ReturnType<typeof getEffectiveConfigSnapshot>;
  projectId: string | null;
  pipeline: EffectivePipelineSnapshot;
}

export function useAudioRecorder({ inputSource, onSegment }: UseAudioRecorderProps) {
  const config = useConfigStore((state) => state.config);
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const isPaused = useTranscriptRuntimeStore((state) => state.isPaused);
  const setIsRecording = useTranscriptRuntimeStore((state) => state.setIsRecording);
  const setIsPaused = useTranscriptRuntimeStore((state) => state.setIsPaused);
  const setAudioUrl = useTranscriptPlaybackStore((state) => state.setAudioUrl);
  const setAudioFile = useTranscriptPlaybackStore((state) => state.setAudioFile);
  const showError = useDialogStore((state) => state.showError);

  const audioContextRef = useRef<AudioContext | null>(null);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const activeStreamRef = useRef<MediaStream | null>(null);
  const nativeAudioUnlistenRef = useRef<(() => void) | null>(null);
  const usingNativeCaptureRef = useRef(false);
  const mimeTypeRef = useRef<string>('');
  const peakLevelRef = useRef<number>(0);
  const activeInputSourceRef = useRef<InputSource>(inputSource);
  const onSegmentRef = useRef(onSegment);
  const recordSessionIdRef = useRef<string | null>(null);
  const recordSessionPhaseRef = useRef<RecordSessionPhase>('idle');
  const recordedDurationMsRef = useRef(0);
  const activeDurationStartedAtRef = useRef<number | null>(null);
  const finalizedDurationSecondsRef = useRef<number | null>(null);
  const segmentTimeOffsetSecondsRef = useRef(0);
  const recordTimelineCursorSecondsRef = useRef(0);
  const liveDraftRef = useRef<LiveRecordingDraftHandle | null>(null);
  const recordingTranslationDrainsRef = useRef(new Map<string, RecordingTranslationDrain>());
  const stopPromiseRef = useRef<Promise<void> | null>(null);
  const pendingDiscardRef = useRef(false);
  const stopRecordingRef = useRef<((options?: { discard?: boolean }) => Promise<void>) | null>(
    null
  );
  const pendingRecordingSnapshotRef = useRef<RecordingAutomationSnapshot | null>(null);
  const recordingSnapshotsRef = useRef(new Map<string, RecordingAutomationSnapshot>());

  const [isInitializing, setIsInitializing] = useState(false);
  const [isTransitioning, setIsTransitioning] = useState(false);
  const [recordingElapsedMs, setRecordingElapsedMs] = useState(0);

  const isRecordingRef = useRef(isRecording);
  const isPausedRef = useRef(isPaused);

  useEffect(() => {
    isRecordingRef.current = isRecording;
  }, [isRecording]);

  useEffect(() => {
    isPausedRef.current = isPaused;
  }, [isPaused]);

  useEffect(() => {
    onSegmentRef.current = onSegment;
  }, [onSegment]);

  const setPeakFromInt16 = useCallback((samples: Int16Array) => {
    let maxAbs = 0;
    for (let i = 0; i < samples.length; i++) {
      const abs = Math.abs(samples[i]);
      if (abs > maxAbs) {
        maxAbs = abs;
      }
    }
    peakLevelRef.current = Math.min(1, maxAbs / 32767);
  }, []);

  const timing = useMemo(
    () =>
      createRecordTimingController({
        refs: {
          recordedDurationMsRef,
          activeDurationStartedAtRef,
          finalizedDurationSecondsRef,
          segmentTimeOffsetSecondsRef,
          recordTimelineCursorSecondsRef,
        },
        logger,
        setRecordingElapsedMs,
        getSessionId: () => recordSessionIdRef.current,
      }),
    []
  );

  const session = useMemo(
    () =>
      createRecordSessionController({
        refs: {
          recordSessionIdRef,
          recordSessionPhaseRef,
          peakLevelRef,
        },
        logger,
        clearSegments: clearTranscriptSegments,
        resetLiveTimingState: timing.resetLiveTimingState,
        clearFinalizedDurationSeconds: timing.clearFinalizedDurationSeconds,
        beginRecordedDurationWindow: timing.beginRecordedDurationWindow,
        syncRecordingElapsedMs: timing.syncRecordingElapsedMs,
        setIsRecording,
        setIsPaused,
        setIsTransitioning,
        getInputSource: () => inputSource,
        getIsRecording: () => isRecordingRef.current,
        softStopRecordRuntime: () => transcriptionService.softStop(),
      }),
    [
      inputSource,
      setIsPaused,
      setIsRecording,
      timing.beginRecordedDurationWindow,
      timing.clearFinalizedDurationSeconds,
      timing.resetLiveTimingState,
      timing.syncRecordingElapsedMs,
    ]
  );

  const persistence = useMemo(
    () =>
      createRecordingPersistence({
        logger,
        history: {
          createLiveRecordingDraft: async (...args) => {
            const snapshot = pendingRecordingSnapshotRef.current;
            const draft = await historyService.createLiveRecordingDraft(...args);
            if (snapshot) recordingSnapshotsRef.current.set(draft.item.id, snapshot);
            return draft;
          },
          completeLiveRecordingDraft: (...args) =>
            historyService.completeLiveRecordingDraft(...args),
          discardLiveRecordingDraft: async (...args) => {
            try {
              await historyService.discardLiveRecordingDraft(...args);
            } finally {
              recordingSnapshotsRef.current.delete(args[0]);
            }
          },
          saveRecording: (...args) => historyService.saveRecording(...args),
          saveNativeRecording: (...args) => historyService.saveNativeRecording(...args),
          loadTranscript: (...args) => historyService.loadTranscript(...args),
          updateTranscript: (...args) => historyService.updateTranscript(...args),
        },
        getTranscriptState: () => {
          const recSessionId =
            recordSessionIdRef.current || useTranscriptStore.getState().recordingSessionId;
          const sessions = useTranscriptStore.getState().sessions;
          const recordingSegments = recSessionId ? sessions[recSessionId]?.segments : undefined;
          return {
            config:
              (recSessionId
                ? recordingSnapshotsRef.current.get(recSessionId)?.config
                : undefined) ??
              pendingRecordingSnapshotRef.current?.config ??
              getEffectiveConfigSnapshot(),
            segments: recordingSegments ?? useTranscriptSessionStore.getState().segments,
            setAudioUrl: (url: string | null) => {
              if (recSessionId) {
                useTranscriptStore.setState((state) => ({
                  sessions: {
                    ...state.sessions,
                    [recSessionId]: {
                      ...(state.sessions[recSessionId] || DEFAULT_SESSION_DATA),
                      audioUrl: url,
                    },
                  },
                }));
                if (useTranscriptStore.getState().activeSessionId === recSessionId) {
                  useTranscriptPlaybackStore.getState().setAudioUrl(url);
                }
              } else {
                useTranscriptPlaybackStore.getState().setAudioUrl(url);
              }
            },
            setSegments: (segments: TranscriptSegment[]) => {
              if (recSessionId) {
                useTranscriptStore.getState().setSegmentsForSession(recSessionId, segments);
                if (useTranscriptStore.getState().activeSessionId === recSessionId) {
                  setTranscriptSegments(segments);
                }
              } else {
                setTranscriptSegments(segments);
              }
            },
            activeSessionId:
              recordSessionIdRef.current ||
              useTranscriptStore.getState().recordingSessionId ||
              useTranscriptStore.getState().activeSessionId,
            setSegmentsForSession: (sessionId: string, segments: TranscriptSegment[]) => {
              useTranscriptStore.getState().setSegmentsForSession(sessionId, segments);
            },
          };
        },
        getActiveProjectId: () =>
          pendingRecordingSnapshotRef.current?.projectId ??
          useProjectStore.getState().activeProjectId,
        addHistoryItem: (item) => useHistoryStore.getState().addItem(item),
        upsertHistoryItem: (item) => useHistoryStore.getState().upsertItem(item),
        removeHistoryItem: (id) =>
          useHistoryStore.setState((state) => ({
            items: state.items.filter((item) => item.id !== id),
          })),
        persistSummary: (historyId) => summaryService.persistSummary(historyId),
        postProcessSavedItem: async (historyId, segments, explicitProjectId) => {
          const originSessionId =
            Object.entries(useTranscriptStore.getState().sessions).find(
              ([id, session]) => id === historyId || session.sourceHistoryId === historyId
            )?.[0] ?? historyId;
          const snapshot = recordingSnapshotsRef.current.get(originSessionId);
          recordingSnapshotsRef.current.delete(originSessionId);
          const session = originSessionId
            ? useTranscriptStore.getState().sessions[originSessionId]
            : undefined;
          const resolvedProjectId =
            explicitProjectId ?? session?.projectId ?? snapshot?.projectId ?? null;
          const effectiveConfig = snapshot?.config ?? useConfigStore.getState().config;
          const pipeline =
            session?.pipelineSnapshot ??
            (snapshot && resolvedProjectId === snapshot.projectId
              ? snapshot.pipeline
              : resolveItemPipeline(
                  resolvedProjectId,
                  useProjectStore.getState().projects,
                  effectiveConfig
                ));
          const drain = recordingTranslationDrainsRef.current.get(originSessionId);
          // Transfer ownership to this saved recording before yielding to another recording.
          recordingTranslationDrainsRef.current.delete(originSessionId);
          await waitForRecordingSegmentTasks(originSessionId);
          if (drain) {
            try {
              await drain.complete;
              const target = createTaskTarget({ sessionId: originSessionId, historyId });
              await runWithDocumentLock(
                target,
                { taskType: 'liveTranslation', lockEditor: false },
                async () => {
                  const latest = await resolveLatestSegmentsForTarget(target, segments, {
                    allowEmpty: true,
                  });
                  segments = mergeRecordingTranslations(latest, drain.translations);
                  if (segments.some((segment, index) => segment !== latest[index])) {
                    await useHistoryStore
                      .getState()
                      .commitSystemTranscriptUpdate(historyId, segments);
                  }
                }
              );
            } finally {
              drain.dispose();
            }
          }
          const hasAnyPostProcess =
            pipeline.autoPolish ||
            pipeline.autoTranslate ||
            pipeline.autoSummary ||
            (pipeline.enabled && pipeline.autoExport);
          if (!hasAnyPostProcess) {
            return segments;
          }
          try {
            const result = await pipelineExecutionEngine.execute({
              historyId,
              sessionId: originSessionId,
              projectId: resolvedProjectId,
              segments,
              pipeline,
              globalConfig: effectiveConfig,
              baseFileName: `Recording-${historyId}`,
              onSegmentsUpdated: (updatedSegments) => {
                setSegmentsForHistory(historyId, updatedSegments);
              },
            });
            return result.segments;
          } catch (error) {
            logger.error('[useAudioRecorder] Post-process pipeline failed:', error);
            return segments;
          }
        },
        commitTranscriptUpdate: async (historyId, segments) => {
          await useHistoryStore.getState().commitSystemTranscriptUpdate(historyId, segments);
        },
        annotateSegmentsForFile: (filePath, segments, transcriptConfig) =>
          speakerService.annotateSegmentsForFile(filePath, segments, transcriptConfig),
        syncSavedRecordingMeta: (title, historyId, icon) => {
          const originSessionId =
            recordSessionIdRef.current ||
            useTranscriptStore.getState().recordingSessionId ||
            undefined;
          useTranscriptStore
            .getState()
            .syncSavedRecordingMeta(title, historyId, icon, undefined, originSessionId);
        },
        writeFile: (filePath, contents) => writeFile(filePath, contents),
        removeFile: (filePath) => audioDeleteRecordingFile(filePath),
        fileSrcFromPath: convertManagedAudioFileSrc,
      }),
    []
  );

  const handleWebRecordingStop = useCallback(
    async (blob: Blob, mimeType: string) => {
      const persistedBlob = blob.type ? blob : new Blob([blob], { type: mimeType });
      const liveDraft = liveDraftRef.current;
      if (!liveDraft) {
        timing.clearFinalizedDurationSeconds();
        return;
      }
      try {
        await persistence.persistBrowserRecording(
          liveDraft,
          persistedBlob,
          timing.getFinalizedDurationSeconds() ?? timing.getRecordedDurationSeconds()
        );
      } finally {
        liveDraftRef.current = null;
        timing.clearFinalizedDurationSeconds();
      }
    },
    [persistence, timing]
  );

  const forwardRecordSegment = useCallback(
    (update: TranscriptUpdate) => {
      const normalizedUpdate = timing.normalizeRecordTranscriptUpdate(update);
      const representativeSegment =
        normalizedUpdate.upsertSegments[normalizedUpdate.upsertSegments.length - 1];
      const meta = session.getRecordSegmentDeliveryMeta(
        representativeSegment ||
          ({
            id: 'record-update-empty',
            text: '',
            start: 0,
            end: 0,
            isFinal: false,
          } as TranscriptSegment)
      );

      if (meta.accepted) {
        timing.trackAcceptedTranscriptUpdate(normalizedUpdate);
      }

      logger.info(
        `[useAudioRecorder] Record update received. session=${meta.sessionId ?? 'none'} phase=${meta.phase} accepted=${meta.accepted} removes=${normalizedUpdate.removeIds.length} upserts=${normalizedUpdate.upsertSegments.length} offset=${segmentTimeOffsetSecondsRef.current.toFixed(3)}`
      );
      onSegmentRef.current(normalizedUpdate, {
        ...meta,
        config: meta.sessionId
          ? recordingSnapshotsRef.current.get(meta.sessionId)?.config
          : undefined,
      });
    },
    [session, timing]
  );
  const handleCaptureError = useCallback(
    (error: string) => {
      void showError({
        code: 'audio.capture_error',
        messageKey: 'errors.audio.capture_failed',
        cause: error,
      });
      setIsPaused(false);
      // Keep `isRecording` active until `stopRecording()` finishes finalizing and archiving
      // the live draft so Channel 1 locks hold and no duplicate recording starts mid-flush.
      void stopRecordingRef.current?.().finally(() => {
        setIsRecording(false);
      });
    },
    [showError, setIsRecording, setIsPaused]
  );

  const capture = useMemo(
    () =>
      createAudioRecorderCapture({
        refs: {
          audioContextRef,
          mediaRecorderRef,
          activeStreamRef,
          nativeAudioUnlistenRef,
          usingNativeCaptureRef,
          mimeTypeRef,
          peakLevelRef,
          activeInputSourceRef,
          isPausedRef,
          recordSessionPhaseRef,
        },
        logger,
        onSegment: forwardRecordSegment,
        onCaptureError: handleCaptureError,
        activateRecordSession: session.activateRecordSession,
        canMutateActiveRecordResources: session.canMutateActiveRecordResources,
        rollbackRecognizer: session.softStopRecordSessionIfActive,
        setPeakFromInt16,
        onWebRecordingStop: handleWebRecordingStop,
        setIsRecording,
        setIsPaused,
      }),
    [
      forwardRecordSegment,
      handleWebRecordingStop,
      session,
      setIsPaused,
      setIsRecording,
      setPeakFromInt16,
      handleCaptureError,
    ]
  );

  const recordController = useMemo(() => {
    const controller = createRecordController({
      logger,
      config,
      inputSource,
      activeInputSourceRef,
      usingNativeCaptureRef,
      liveDraftRef,
      setAudioUrl,
      setAudioFile,
      setIsInitializing,
      setIsTransitioning,
      setIsPaused,
      showError,
      capture,
      session,
      timing,
      persistence,
    });
    return controller;
  }, [
    capture,
    config,
    inputSource,
    persistence,
    session,
    setAudioFile,
    setAudioUrl,
    setIsPaused,
    showError,
    timing,
  ]);

  useEffect(() => {
    if (!isRecording || isPaused) {
      return;
    }

    timing.syncRecordingElapsedMs();
    const interval = window.setInterval(timing.syncRecordingElapsedMs, 250);
    return () => window.clearInterval(interval);
  }, [isPaused, isRecording, timing]);

  const startRecording = useCallback(
    async (explicitProjectId?: string | null) => {
      const targetProjectId =
        explicitProjectId !== undefined
          ? explicitProjectId
          : useProjectStore.getState().activeProjectId;
      const effectiveConfig = useConfigStore.getState().config;
      const pipeline = resolveItemPipeline(
        targetProjectId,
        useProjectStore.getState().projects,
        effectiveConfig
      );
      pendingRecordingSnapshotRef.current = structuredClone({
        projectId: targetProjectId,
        pipeline,
        config: effectiveConfig,
      });
      const activeRecSessionId =
        recordSessionIdRef.current ?? useTranscriptStore.getState().recordingSessionId;
      if (activeRecSessionId) {
        useTranscriptStore
          .getState()
          .setSessionProjectContext(activeRecSessionId, targetProjectId, pipeline);
      }
      const asrRequest = resolveAsrTranscriptionRequest(effectiveConfig, 'live');
      if (!isAsrRequestConfigured(asrRequest)) {
        if (asrRequest.engine === 'online') {
          void showError({
            code: 'asr.not_configured',
            messageKey: 'batch.no_model_error',
          });
          return false;
        }
        const onboardingStore = useOnboardingStore.getState();
        useTranscriptRuntimeStore.getState().setMode('create', 'live');
        onboardingStore.reopen(
          getResumeOnboardingStep(effectiveConfig, 'live_record', onboardingStore.persistedState),
          'live_record'
        );
        return false;
      }
      transcriptionService.setProjectId(targetProjectId);
      peakLevelRef.current = 0;
      return recordController.startRecording();
    },
    [recordController, showError]
  );

  const stopRecording = useCallback(
    async (options?: { discard?: boolean }) => {
      if (stopPromiseRef.current) {
        if (options?.discard) pendingDiscardRef.current = true;
        return stopPromiseRef.current;
      }
      const sessionId = recordSessionIdRef.current;
      if (!sessionId) return;
      const stopping = (async () => {
        const drain = await collectRecordingTranslations(sessionId);
        recordingTranslationDrainsRef.current.set(sessionId, drain);
        try {
          await recordController.stopRecording(
            pendingDiscardRef.current ? { discard: true } : options
          );
        } finally {
          // `postProcessSavedItem` claims the drain synchronously inside the awaited stop and
          // owns its disposal; anything still registered here was never claimed (discarded or
          // empty recording) and must not keep its listener or quit-blocking task alive.
          const unclaimed = recordingTranslationDrainsRef.current.get(sessionId);
          if (unclaimed) {
            unclaimed.dispose();
            recordingTranslationDrainsRef.current.delete(sessionId);
          }
        }
      })();
      stopPromiseRef.current = stopping;
      try {
        await stopping;
      } finally {
        pendingDiscardRef.current = false;
        recordingTranslationDrainsRef.current.delete(sessionId);
        recordingSnapshotsRef.current.delete(sessionId);
        stopPromiseRef.current = null;
      }
    },
    [recordController]
  );

  stopRecordingRef.current = stopRecording;

  const pauseRecording = useCallback(async () => {
    peakLevelRef.current = 0;
    return recordController.pauseRecording();
  }, [recordController]);

  const resumeRecording = useCallback(async () => {
    return recordController.resumeRecording();
  }, [recordController]);

  useEffect(() => {
    if (isRecording) {
      registerActiveRecorderStopHandler(stopRecording);
      return () => {
        registerActiveRecorderStopHandler(null);
      };
    }
    registerActiveRecorderStopHandler(null);
  }, [isRecording, stopRecording]);

  useEffect(() => {
    return () => {
      registerActiveRecorderStopHandler(null);
      void capture.teardownWebCaptureResources().catch((error) => {
        logger.error('Error closing audio recorder web capture resources:', error);
      });
      transcriptionService
        .terminate()
        .catch((error) => logger.error('Error stopping transcription service:', error));
    };
  }, [capture]);
  return {
    startRecording,
    stopRecording,
    pauseRecording,
    resumeRecording,
    isRecording,
    isPaused,
    isInitializing,
    isTransitioning,
    recordingElapsedMs,
    peakLevelRef,
    setRecordSessionId: session.setRecordSessionId,
  };
}
