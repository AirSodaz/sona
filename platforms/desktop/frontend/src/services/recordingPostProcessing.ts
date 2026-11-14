import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { logger } from '../utils/logger';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import { trackQuitBlockingTask } from './quitGuard';
import { listen } from './tauri/platform/events';

const RECORDING_TRANSLATION_DRAIN_TIMEOUT_MS = 30_000;

const pendingSegmentTasks = new Map<string, Set<Promise<unknown>>>();

export function trackRecordingSegmentTask(sessionId: string, task: Promise<unknown>): void {
  const tasks = pendingSegmentTasks.get(sessionId) ?? new Set<Promise<unknown>>();
  pendingSegmentTasks.set(sessionId, tasks);
  const tracked = trackQuitBlockingTask(task);
  tasks.add(tracked);
  const cleanup = () => {
    tasks.delete(tracked);
    if (tasks.size === 0) pendingSegmentTasks.delete(sessionId);
  };
  void tracked.then(cleanup, cleanup);
}

export async function waitForRecordingSegmentTasks(sessionId: string): Promise<void> {
  await Promise.allSettled(pendingSegmentTasks.get(sessionId) ?? []);
}

interface RecordingTranslationEvent {
  sessionId: string;
  stage: string;
  update: TranscriptUpdate;
}

export interface RecordingTranslationDrain {
  complete: Promise<void>;
  translations: Map<string, TranscriptSegment>;
  dispose: () => void;
}

/** Installed before stop so even an immediate completion cannot race listener registration. */
export async function collectRecordingTranslations(
  sessionId: string
): Promise<RecordingTranslationDrain> {
  const translations = new Map<string, TranscriptSegment>();
  const completion = Promise.withResolvers<void>();
  let disposed = false;
  let unlisten: (() => void) | undefined;
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    clearTimeout(timeoutId);
    unlisten?.();
    completion.resolve();
  };
  timeoutId = setTimeout(() => {
    logger.warn('[recordingPostProcessing] Translation drain timed out', { sessionId });
    dispose();
  }, RECORDING_TRANSLATION_DRAIN_TIMEOUT_MS);
  try {
    unlisten = await listen<RecordingTranslationEvent>(
      `recording-translation-${sessionId}`,
      ({ payload }) => {
        if (disposed || payload.sessionId !== sessionId) return;
        if (payload.stage === 'background_complete') {
          dispose();
          return;
        }
        for (const segment of normalizeTranscriptUpdate(payload.update).upsertSegments) {
          if (segment.translation) translations.set(segment.id, segment);
        }
      }
    );
  } catch (error) {
    logger.warn('[recordingPostProcessing] Translation drain listener failed', {
      sessionId,
      error,
    });
  }
  if (disposed) unlisten?.();
  return { complete: trackQuitBlockingTask(completion.promise), translations, dispose };
}

/** Late translations only patch unchanged source text, preserving edits and speaker metadata. */
export function mergeRecordingTranslations(
  segments: TranscriptSegment[],
  translations: Map<string, TranscriptSegment>
): TranscriptSegment[] {
  return segments.map((segment) => {
    const translated = translations.get(segment.id);
    if (
      !translated ||
      translated.text !== segment.text ||
      (translated.translation === segment.translation && !segment.draftTranslation)
    )
      return segment;
    return { ...segment, translation: translated.translation, draftTranslation: undefined };
  });
}
