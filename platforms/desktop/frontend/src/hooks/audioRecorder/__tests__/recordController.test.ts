import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LiveRecordingDraftHandle } from '../../../services/historyService';
import {
  registerActiveRecorderStopHandler,
  stopActiveRecording,
} from '../../../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../../../stores/transcriptSessionStore';
import { createRecordController } from '../recordController';
import type { MutableRefLike } from '../types';

vi.mock('@tauri-apps/plugin-fs', () => ({
  remove: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../../services/transcriptionService', () => ({
  transcriptionService: {
    softStop: vi.fn().mockResolvedValue(undefined),
    stopNativeCapture: vi.fn().mockResolvedValue('C:/history/test-rec.wav'),
  },
}));

vi.mock('../../../services/historyService', () => ({
  flushPendingAutoSave: vi.fn().mockResolvedValue(undefined),
}));

function createRef<T>(value: T): MutableRefLike<T> {
  return { current: value };
}

function createDraftHandle(id: string): LiveRecordingDraftHandle {
  return {
    item: {
      id,
      timestamp: 1,
      duration: 0,
      audioPath: `${id}.wav`,
      transcriptPath: `${id}.json`,
      title: `Recording ${id}`,
      previewText: '',
      projectId: 'project-1',
      icon: 'system:mic',
      type: 'recording',
      searchContent: '',
      status: 'draft',
      draftSource: 'live_record',
    },
    audioAbsolutePath: `C:/history/${id}.wav`,
  };
}

describe('recordController stopRecording and coordinator delegation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTranscriptSessionStore.setState({
      segments: [{ id: 'seg-1', text: 'Hello world', start: 0, end: 2, isFinal: true }],
    });
  });

  it('finalizes native recording draft with WAV path when stopped normally', async () => {
    const liveDraft = createDraftHandle('draft-1');
    const usingNativeCaptureRef = createRef(true);
    const liveDraftRef = createRef<LiveRecordingDraftHandle | null>(liveDraft);
    const persistNativeRecording = vi.fn().mockResolvedValue(undefined);
    const discardLiveRecordingDraft = vi.fn().mockResolvedValue(undefined);

    const controller = createRecordController({
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      config: { muteDuringRecording: false } as any,
      inputSource: 'microphone',
      activeInputSourceRef: createRef('microphone'),
      usingNativeCaptureRef,
      liveDraftRef,
      setAudioUrl: vi.fn(),
      setAudioFile: vi.fn(),
      setIsInitializing: vi.fn(),
      setIsTransitioning: vi.fn(),
      setIsPaused: vi.fn(),
      showError: vi.fn().mockResolvedValue(undefined),
      capture: {
        tryStartNativeDesktopCapture: vi.fn().mockResolvedValue(true),
        stopFileRecording: vi.fn().mockResolvedValue(undefined),
        teardownWebCaptureResources: vi.fn().mockResolvedValue(undefined),
        stopCaptureForSession: vi.fn().mockResolvedValue('C:/history/draft-1.wav'),
        setSystemAudioMute: vi.fn().mockResolvedValue(undefined),
      } as any,
      session: {
        openRecordSession: () => 'rec-session-1',
        softStopRecordSessionIfActive: vi.fn().mockResolvedValue(undefined),
        resetRecordSession: vi.fn(),
        getSessionId: () => 'rec-session-1',
        getPhase: () => 'recording',
        setPhase: vi.fn(),
        isActiveSession: () => true,
        shouldFinalizeRecordStartAttempt: () => true,
      } as any,
      timing: {
        clearFinalizedDurationSeconds: vi.fn(),
        getFinalizedDurationSeconds: () => 5.2,
        getRecordedDurationSeconds: () => 5.2,
        finalizeRecordedDurationSeconds: () => 5.2,
      } as any,
      persistence: {
        createLiveRecordingDraft: vi.fn(),
        discardLiveRecordingDraft,
        persistNativeRecording,
      },
    });

    registerActiveRecorderStopHandler(controller.stopRecording);
    const stopped = await stopActiveRecording();

    expect(stopped).toBe(true);
    expect(persistNativeRecording).toHaveBeenCalledWith(liveDraft, 'C:/history/draft-1.wav', 5.2);
    expect(discardLiveRecordingDraft).not.toHaveBeenCalled();
    expect(liveDraftRef.current).toBeNull();
    expect(usingNativeCaptureRef.current).toBe(false);
  });

  it('discards native recording draft when discard option is requested via coordinator', async () => {
    const liveDraft = createDraftHandle('draft-2');
    const usingNativeCaptureRef = createRef(true);
    const liveDraftRef = createRef<LiveRecordingDraftHandle | null>(liveDraft);
    const persistNativeRecording = vi.fn().mockResolvedValue(undefined);
    const discardLiveRecordingDraft = vi.fn().mockResolvedValue(undefined);

    const controller = createRecordController({
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      config: { muteDuringRecording: false } as any,
      inputSource: 'microphone',
      activeInputSourceRef: createRef('microphone'),
      usingNativeCaptureRef,
      liveDraftRef,
      setAudioUrl: vi.fn(),
      setAudioFile: vi.fn(),
      setIsInitializing: vi.fn(),
      setIsTransitioning: vi.fn(),
      setIsPaused: vi.fn(),
      showError: vi.fn().mockResolvedValue(undefined),
      capture: {
        tryStartNativeDesktopCapture: vi.fn().mockResolvedValue(true),
        stopFileRecording: vi.fn().mockResolvedValue(undefined),
        teardownWebCaptureResources: vi.fn().mockResolvedValue(undefined),
        stopCaptureForSession: vi.fn().mockResolvedValue('C:/history/draft-2.wav'),
        setSystemAudioMute: vi.fn().mockResolvedValue(undefined),
      } as any,
      session: {
        openRecordSession: () => 'rec-session-2',
        softStopRecordSessionIfActive: vi.fn().mockResolvedValue(undefined),
        resetRecordSession: vi.fn(),
        getSessionId: () => 'rec-session-2',
        getPhase: () => 'recording',
        setPhase: vi.fn(),
        isActiveSession: () => true,
        shouldFinalizeRecordStartAttempt: () => true,
      } as any,
      timing: {
        clearFinalizedDurationSeconds: vi.fn(),
        getFinalizedDurationSeconds: () => 3.1,
        getRecordedDurationSeconds: () => 3.1,
        finalizeRecordedDurationSeconds: () => 3.1,
      } as any,
      persistence: {
        createLiveRecordingDraft: vi.fn(),
        discardLiveRecordingDraft,
        persistNativeRecording,
      },
    });

    registerActiveRecorderStopHandler(controller.stopRecording);
    const stopped = await stopActiveRecording({ discard: true });

    expect(stopped).toBe(true);
    expect(discardLiveRecordingDraft).toHaveBeenCalledWith(liveDraft);
    expect(persistNativeRecording).not.toHaveBeenCalled();
    expect(liveDraftRef.current).toBeNull();
  });
});
