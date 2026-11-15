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
  sessionId: string;
  complete: Promise<void>;
  translations: Map<string, TranscriptSegment>;
  /** Segment ids the app rewrote after translation; alive as long as this drain. */
  systemRewritten: ReadonlySet<string>;
  dispose: () => void;
}

/**
 * Segment ids whose text the app rewrote (auto-polish) after the live translation was
 * computed. Only these accept a drained translation whose captured source text no longer
 * matches, so user edits keep their protection. Scoped to the recording session's drain.
 */
const systemRewrittenSegments = new Map<string, Set<string>>();

export function markRecordingSegmentSystemRewrite(sessionId: string, segmentId: string): void {
  const ids = systemRewrittenSegments.get(sessionId) ?? new Set<string>();
  systemRewrittenSegments.set(sessionId, ids);
  ids.add(segmentId);
}

/** Installed before stop so even an immediate completion cannot race listener registration. */
export async function collectRecordingTranslations(
  sessionId: string
): Promise<RecordingTranslationDrain> {
  const translations = new Map<string, TranscriptSegment>();
  // Same instance the marker writes into, so rewrites recorded after this point still land.
  const systemRewritten = systemRewrittenSegments.get(sessionId) ?? new Set<string>();
  systemRewrittenSegments.set(sessionId, systemRewritten);
  const completion = Promise.withResolvers<void>();
  let disposed = false;
  let completed = false;
  let unlisten: (() => void) | undefined;
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  /** Ends the wait but keeps `systemRewritten` alive for the merge that follows. */
  const settle = () => {
    if (disposed || completed) return;
    completed = true;
    clearTimeout(timeoutId);
    unlisten?.();
    completion.resolve();
  };
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    clearTimeout(timeoutId);
    unlisten?.();
    systemRewrittenSegments.delete(sessionId);
    completion.resolve();
  };
  timeoutId = setTimeout(() => {
    logger.warn('[recordingPostProcessing] Translation drain timed out', { sessionId });
    settle();
  }, RECORDING_TRANSLATION_DRAIN_TIMEOUT_MS);
  try {
    unlisten = await listen<RecordingTranslationEvent>(
      `recording-translation-${sessionId}`,
      ({ payload }) => {
        if (disposed || completed || payload.sessionId !== sessionId) return;
        if (payload.stage === 'background_complete') {
          settle();
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
    // No completion sentinel can arrive without a listener; finish now instead of
    // stalling post-processing and the quit guard for the full timeout.
    settle();
  }
  if (completed) unlisten?.();
  return {
    sessionId,
    complete: trackQuitBlockingTask(completion.promise),
    translations,
    systemRewritten,
    dispose,
  };
}

/** Late translations only patch unchanged source text, preserving edits and speaker metadata. */
export function mergeRecordingTranslations(
  segments: TranscriptSegment[],
  drain: Pick<RecordingTranslationDrain, 'translations' | 'systemRewritten'>
): TranscriptSegment[] {
  return segments.map((segment) => {
    const translated = drain.translations.get(segment.id);
    if (!translated) return segment;
    if (translated.text !== segment.text && !drain.systemRewritten.has(segment.id)) return segment;
    if (translated.translation === segment.translation && !segment.draftTranslation) {
      return segment;
    }
    return { ...segment, translation: translated.translation, draftTranslation: undefined };
  });
}
