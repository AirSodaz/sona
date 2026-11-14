import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TranscriptSegment } from '../../types/transcript';
import {
  collectRecordingTranslations,
  mergeRecordingTranslations,
  trackRecordingSegmentTask,
  waitForRecordingSegmentTasks,
} from '../recordingPostProcessing';

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  unlisten: vi.fn(),
  track: vi.fn(<T>(promise: Promise<T>) => promise),
}));
vi.mock('../tauri/platform/events', () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
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
    const result = mergeRecordingTranslations(
      [original, edited],
      new Map([
        ['a', translated],
        ['b', { ...translated, id: 'b' }],
        ['deleted', { ...translated, id: 'deleted' }],
      ])
    );
    expect(result).toEqual([{ ...original, translation: '你好' }, edited]);
    expect(result[1]).toBe(edited);
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
