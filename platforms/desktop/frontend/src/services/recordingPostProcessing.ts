import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { logger } from '../utils/logger';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import { trackQuitBlockingTask } from './quitGuard';
import { buildRecordingTranslationEvent } from './tauri/events';
import { listen } from './tauri/platform/events';

const RECORDING_TRANSLATION_DRAIN_TIMEOUT_MS = 30_000;
const RECORDING_SEGMENT_TASK_TIMEOUT_MS = 15_000;

interface RecordingSegmentTask {
  complete: Promise<void>;
  release: () => void;
}

const pendingSegmentTasks = new Map<string, Set<RecordingSegmentTask>>();

export function trackRecordingSegmentTask(sessionId: string, task: Promise<unknown>): void {
  const tasks = pendingSegmentTasks.get(sessionId) ?? new Set<RecordingSegmentTask>();
  pendingSegmentTasks.set(sessionId, tasks);
  const completion = Promise.withResolvers<void>();
  let released = false;
  const tracked: RecordingSegmentTask = {
    complete: trackQuitBlockingTask(completion.promise),
    release: () => {
      if (released) return;
      released = true;
      clearTimeout(timeoutId);
      completion.resolve();
      tasks.delete(tracked);
      if (tasks.size === 0) pendingSegmentTasks.delete(sessionId);
    },
  };
  // The quit guard must settle even if the original job never responds, including for a
  // discarded recording whose post-processing never calls waitForRecordingSegmentTasks.
  const timeoutId = setTimeout(() => {
    logger.warn('[recordingPostProcessing] Segment task timed out', { sessionId });
    tracked.release();
  }, RECORDING_SEGMENT_TASK_TIMEOUT_MS);
  tasks.add(tracked);
  void task.then(tracked.release, tracked.release);
}

/**
 * Waits for the auto-polish tasks a session started. Bounded: a task whose job response never
 * arrives must not keep the saved recording's post-processing (and the quit guard) pending.
 */
export async function waitForRecordingSegmentTasks(
  sessionId: string,
  timeoutMs = RECORDING_SEGMENT_TASK_TIMEOUT_MS
): Promise<void> {
  const tasks = pendingSegmentTasks.get(sessionId);
  if (!tasks || tasks.size === 0) return;
  const pending = [...tasks];
  const timeoutId = setTimeout(() => {
    logger.warn('[recordingPostProcessing] Segment task wait timed out', { sessionId });
    for (const task of pending) task.release();
  }, timeoutMs);
  try {
    await Promise.all(pending.map((task) => task.complete));
  } finally {
    clearTimeout(timeoutId);
  }
}

interface RecordingTranslationEvent {
  sessionId: string;
  stage: string;
  update: TranscriptUpdate;
  timeoutMs?: number;
}

export interface RecordingTranslationDrain {
  sessionId: string;
  complete: Promise<void>;
  translations: Map<string, TranscriptSegment>;
  /** Segment id → text the app substituted; alive as long as this drain. */
  systemRewritten: ReadonlyMap<string, string>;
  dispose: () => void;
}

/**
 * Segment ids the app rewrote (auto-polish) after the live translation was computed, mapped to
 * the text it substituted. A drained translation whose captured source text no longer matches
 * is accepted only while the segment still holds that rewritten text, so a later user edit
 * keeps its protection. Scoped to the recording session's drain.
 */
const systemRewrittenSegments = new Map<string, Map<string, string>>();

/**
 * Sessions whose drain is gone. Polish keeps streaming after a discard or after the bounded
 * wait, and a late marker would otherwise recreate `systemRewrittenSegments` forever.
 * Bounded: only the most recent sessions can still receive a late marker.
 */
const closedDrainSessions = new Set<string>();
const MAX_CLOSED_DRAIN_SESSIONS = 32;

function closeDrainSession(sessionId: string): void {
  closedDrainSessions.delete(sessionId);
  closedDrainSessions.add(sessionId);
  if (closedDrainSessions.size > MAX_CLOSED_DRAIN_SESSIONS) {
    const oldest = closedDrainSessions.values().next().value;
    if (oldest !== undefined) closedDrainSessions.delete(oldest);
  }
}

export function markRecordingSegmentSystemRewrite(
  sessionId: string,
  segmentId: string,
  rewrittenText: string
): void {
  if (closedDrainSessions.has(sessionId)) return;
  const rewrites = systemRewrittenSegments.get(sessionId) ?? new Map<string, string>();
  systemRewrittenSegments.set(sessionId, rewrites);
  rewrites.set(segmentId, rewrittenText);
}

/** Installed before stop so even an immediate completion cannot race listener registration. */
export async function collectRecordingTranslations(
  sessionId: string
): Promise<RecordingTranslationDrain> {
  const translations = new Map<string, TranscriptSegment>();
  // A recording session id is unique, so a tombstone can only belong to a previous run.
  closedDrainSessions.delete(sessionId);
  // Same instance the marker writes into, so rewrites recorded after this point still land.
  const systemRewritten = systemRewrittenSegments.get(sessionId) ?? new Map<string, string>();
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
    closeDrainSession(sessionId);
    completion.resolve();
  };
  const armTimeout = (timeoutMs: number) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => {
      logger.warn('[recordingPostProcessing] Translation drain timed out', { sessionId });
      settle();
    }, timeoutMs);
  };
  armTimeout(RECORDING_TRANSLATION_DRAIN_TIMEOUT_MS);
  try {
    unlisten = await listen<RecordingTranslationEvent>(
      buildRecordingTranslationEvent(sessionId),
      ({ payload }) => {
        if (disposed || completed || payload.sessionId !== sessionId) return;
        if (payload.stage === 'background_started') {
          if (
            typeof payload.timeoutMs === 'number' &&
            Number.isFinite(payload.timeoutMs) &&
            payload.timeoutMs >= 0
          ) {
            // The backend budget covers every pending segment. Keep the original watchdog
            // as transport grace rather than cutting valid work after a fixed 30 seconds.
            armTimeout(payload.timeoutMs + RECORDING_TRANSLATION_DRAIN_TIMEOUT_MS);
          }
          return;
        }
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

/**
 * Late translations only patch unchanged source text and untouched translations, preserving
 * edits and speaker metadata. Pass the snapshot taken before the drain started as
 * `baselineSegments`; a translation the user changed since then wins over the drained one.
 */
export function mergeRecordingTranslations(
  segments: TranscriptSegment[],
  drain: Pick<RecordingTranslationDrain, 'translations' | 'systemRewritten'>,
  baselineSegments: TranscriptSegment[] = segments
): TranscriptSegment[] {
  const baseline = new Map(baselineSegments.map((segment) => [segment.id, segment]));
  return segments.map((segment) => {
    const translated = drain.translations.get(segment.id);
    if (!translated) return segment;
    // Accept a text mismatch only while the segment still holds the app's own rewrite; a user
    // edit (which moves the text away from both) keeps protecting the segment.
    if (
      translated.text !== segment.text &&
      drain.systemRewritten.get(segment.id) !== segment.text
    ) {
      return segment;
    }
    // The user edited or cleared the translation, or is mid-edit: their value wins.
    if (segment.draftTranslation) return segment;
    const saved = baseline.get(segment.id);
    if (saved && saved.translation !== segment.translation) return segment;
    if (translated.translation === segment.translation) return segment;
    return { ...segment, translation: translated.translation, draftTranslation: undefined };
  });
}
