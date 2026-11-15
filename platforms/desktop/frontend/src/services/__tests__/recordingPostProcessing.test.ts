import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TranscriptSegment } from '../../types/transcript';
import {
  collectRecordingTranslations,
  markRecordingSegmentSystemRewrite,
  mergeRecordingTranslations,
  trackRecordingSegmentTask,
  waitForRecordingSegmentTasks,
} from '../recordingPostProcessing';

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  unlisten: vi.fn(),
  track: vi.fn(<T>(promise: Promise<T>) => promise),
  failNextListen: { value: false },
}));
vi.mock('../tauri/platform/events', () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
    if (mocks.failNextListen.value) {
      mocks.failNextListen.value = false;
      throw new Error('listen unavailable');
    }
    mocks.listeners.set(name, listener);
    return () => {
      mocks.listeners.delete(name);
      mocks.unlisten(name);
    };
  }),
}));
vi.mock('../quitGuard', () => ({ trackQuitBlockingTask: mocks.track }));
vi.mock('../../utils/logger', () => ({ logger: { warn: vi.fn() } }));

function emit(sessionId: string, stage: string, segments: TranscriptSegment[] = []) {
  mocks.listeners.get(`recording-translation-${sessionId}`)?.({
    payload: { sessionId, stage, update: { removeIds: [], upsertSegments: segments } },
  });
}

describe('recording translation drain', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listeners.clear();
  });

  it('keeps slow translation separate from stopping and from the next recording', async () => {
    const old = await collectRecordingTranslations('old');
    const next = await collectRecordingTranslations('next');
    const settled = vi.fn();
    void old.complete.then(settled);
    await Promise.resolve();
    expect(settled).not.toHaveBeenCalled();
    emit('old', 'enrichment', [
      { id: 'seg-1', text: 'Hello', start: 0, end: 1, isFinal: true, translation: '你好' },
    ]);
    emit('old', 'background_complete');
    await old.complete;
    expect(old.translations.get('seg-1')?.translation).toBe('你好');
    expect(next.translations.size).toBe(0);
    expect(mocks.listeners.has('recording-translation-old')).toBe(false);
    expect(mocks.listeners.has('recording-translation-next')).toBe(true);
    expect(mocks.track).toHaveBeenCalledTimes(2);
    next.dispose();
  });

  it('merges translations without restoring stale text, timing, removed segments or metadata', () => {
    const original: TranscriptSegment = { id: 'a', text: 'Hello', start: 5, end: 6, isFinal: true };
    const edited = { ...original, id: 'b', text: 'Edited' };
    const translated = { ...original, start: 0, end: 1, translation: '你好' };
    const result = mergeRecordingTranslations([original, edited], {
      translations: new Map([
        ['a', translated],
        ['b', { ...translated, id: 'b' }],
        ['deleted', { ...translated, id: 'deleted' }],
      ]),
      systemRewritten: new Map<string, string>(),
    });
    expect(result).toEqual([{ ...original, translation: '你好' }, edited]);
    expect(result[1]).toBe(edited);
  });

  it('accepts a drained translation across an app rewrite but not across a user edit', async () => {
    const drain = await collectRecordingTranslations('rewritten');
    emit('rewritten', 'enrichment', [
      {
        id: 'polished',
        text: 'hello world',
        start: 0,
        end: 1,
        isFinal: true,
        translation: '你好世界',
      },
      { id: 'edited', text: 'hello world', start: 0, end: 2, isFinal: true, translation: '你好' },
    ]);
    emit('rewritten', 'background_complete');
    await drain.complete;
    // Only the polish rewrite is announced; the other text change is the user's.
    markRecordingSegmentSystemRewrite('rewritten', 'polished', 'Hello, world.');
    const segments: TranscriptSegment[] = [
      { id: 'polished', text: 'Hello, world.', start: 0, end: 1, isFinal: true },
      { id: 'edited', text: 'Changed by hand', start: 0, end: 2, isFinal: true },
    ];
    const result = mergeRecordingTranslations(segments, drain);
    expect(result[0].translation).toBe('你好世界');
    expect(result[1]).toBe(segments[1]);
    drain.dispose();
  });

  it('still protects a segment the user edited after the app rewrote it', async () => {
    const drain = await collectRecordingTranslations('edit-after-rewrite');
    emit('edit-after-rewrite', 'enrichment', [
      {
        id: 'polished',
        text: 'hello world',
        start: 0,
        end: 1,
        isFinal: true,
        translation: '你好世界',
      },
    ]);
    emit('edit-after-rewrite', 'background_complete');
    await drain.complete;
    markRecordingSegmentSystemRewrite('edit-after-rewrite', 'polished', 'Hello, world.');
    const segments: TranscriptSegment[] = [
      { id: 'polished', text: 'Hello, world. (edited by hand)', start: 0, end: 1, isFinal: true },
    ];
    const result = mergeRecordingTranslations(segments, drain);
    expect(result[0]).toBe(segments[0]);
    drain.dispose();
  });

  it('stops waiting for a polish task that never settles', async () => {
    vi.useFakeTimers();
    const stuck = Promise.withResolvers<void>();
    trackRecordingSegmentTask('stuck-polish', stuck.promise);
    let settled = false;
    const waiting = waitForRecordingSegmentTasks('stuck-polish', 1_000).then(() => {
      settled = true;
    });
    await Promise.resolve();
    expect(settled).toBe(false);
    await vi.advanceTimersByTimeAsync(1_000);
    await waiting;
    expect(settled).toBe(true);
    stuck.resolve();
    vi.useRealTimers();
  });

  it('cleans up after empty completion or a discarded recording', async () => {
    const empty = await collectRecordingTranslations('empty');
    emit('empty', 'background_complete');
    await empty.complete;
    expect(empty.translations.size).toBe(0);
    const discarded = await collectRecordingTranslations('discarded');
    discarded.dispose();
    discarded.dispose();
    await discarded.complete;
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);
  });

  it('settles on its own when the completion sentinel never arrives', async () => {
    vi.useFakeTimers();
    const drain = await collectRecordingTranslations('silent');
    const settled = vi.fn();
    void drain.complete.then(settled);
    await Promise.resolve();
    expect(settled).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(settled).toHaveBeenCalledOnce();
    expect(mocks.listeners.has('recording-translation-silent')).toBe(false);
    vi.useRealTimers();
  });

  it('settles immediately when the drain listener cannot be registered', async () => {
    mocks.failNextListen.value = true;
    const drain = await collectRecordingTranslations('unavailable');
    // No sentinel can arrive without a listener, so the drain must not wait out the timeout.
    await drain.complete;
    expect(drain.translations.size).toBe(0);
    expect(mocks.listeners.has('recording-translation-unavailable')).toBe(false);
  });

  it('keeps every translation when the backend drain budget exceeds thirty seconds', async () => {
    vi.useFakeTimers();
    const drain = await collectRecordingTranslations('long-drain');
    try {
      const settled = vi.fn();
      void drain.complete.then(settled);
      mocks.listeners.get('recording-translation-long-drain')?.({
        payload: { sessionId: 'long-drain', stage: 'background_started', timeoutMs: 42_000 },
      });
      for (let index = 0; index < 8; index++) {
        await vi.advanceTimersByTimeAsync(4_000);
        emit('long-drain', 'catchup_translation', [
          {
            id: `seg-${index}`,
            text: 'Hello',
            start: index,
            end: index + 1,
            isFinal: true,
            translation: '你好',
          },
        ]);
      }
      expect(settled).not.toHaveBeenCalled();
      emit('long-drain', 'background_complete');
      await drain.complete;
      expect(drain.translations.size).toBe(8);
    } finally {
      drain.dispose();
      vi.useRealTimers();
    }
  });

  it('retains a bounded watchdog when the backend budget is announced but completion is lost', async () => {
    vi.useFakeTimers();
    const drain = await collectRecordingTranslations('lost-completion');
    try {
      const settled = vi.fn();
      void drain.complete.then(settled);
      mocks.listeners.get('recording-translation-lost-completion')?.({
        payload: { sessionId: 'lost-completion', stage: 'background_started', timeoutMs: 42_000 },
      });
      await vi.advanceTimersByTimeAsync(42_000);
      expect(settled).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(30_000);
      await drain.complete;
      expect(mocks.listeners.has('recording-translation-lost-completion')).toBe(false);
    } finally {
      drain.dispose();
      vi.useRealTimers();
    }
  });

  it('waits for in-flight polish on the old recording without waiting for the next recording', async () => {
    const old = Promise.withResolvers<void>();
    const next = Promise.withResolvers<void>();
    trackRecordingSegmentTask('old-polish', old.promise);
    trackRecordingSegmentTask('next-polish', next.promise);
    const settled = vi.fn();
    const wait = waitForRecordingSegmentTasks('old-polish').then(settled);
    await Promise.resolve();
    expect(settled).not.toHaveBeenCalled();
    old.resolve();
    await wait;
    expect(settled).toHaveBeenCalledOnce();
    next.resolve();
    await waitForRecordingSegmentTasks('next-polish');
  });

  it('settles session tracking when a background polish fails', async () => {
    const polish = Promise.withResolvers<void>();
    trackRecordingSegmentTask('failed-polish', polish.promise);
    const waiting = waitForRecordingSegmentTasks('failed-polish');
    polish.reject(new Error('polish failed'));
    await waiting;
    await waitForRecordingSegmentTasks('failed-polish');
  });
});
