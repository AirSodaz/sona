import { beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_SESSION_DATA, useTranscriptStore } from '../../../stores/transcriptStore';
import type { TranscriptSegment } from '../../../types/transcript';
import { createTaskTarget, resolveLatestSegmentsForTarget } from '../taskTarget';

const savedSegment: TranscriptSegment = {
  id: 'saved-1',
  text: 'Saved text',
  start: 0,
  end: 1,
  isFinal: true,
};

const fallbackSegment: TranscriptSegment = {
  id: 'fallback-1',
  text: 'Stale caller snapshot',
  start: 0,
  end: 1,
  isFinal: true,
};

describe('resolveLatestSegmentsForTarget', () => {
  beforeEach(() => {
    useTranscriptStore.setState({
      activeSessionId: 'default',
      recordingSessionId: null,
      sessions: { default: { ...DEFAULT_SESSION_DATA } },
    });
  });

  it('never prefers an empty resident session just because allowEmpty is set', async () => {
    useTranscriptStore.setState({
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        // The task's own session is bound to another history item, so the bound branch misses.
        live: { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'other-history', segments: [] },
        // An editor session for the requested item exists but its transcript is not loaded yet.
        editor: { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'history-1', segments: [] },
      },
    });

    const segments = await resolveLatestSegmentsForTarget(
      createTaskTarget({ sessionId: 'live', historyId: 'history-1' }),
      [fallbackSegment],
      { allowEmpty: true, loadTranscript: async () => [savedSegment] }
    );

    expect(segments).toEqual([savedSegment]);
  });

  it('keeps the bound session empty when the user cleared it', async () => {
    useTranscriptStore.setState({
      sessions: {
        default: { ...DEFAULT_SESSION_DATA },
        live: { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'history-1', segments: [] },
      },
    });

    const segments = await resolveLatestSegmentsForTarget(
      createTaskTarget({ sessionId: 'live', historyId: 'history-1' }),
      [savedSegment],
      { allowEmpty: true, loadTranscript: async () => [savedSegment] }
    );

    expect(segments).toEqual([]);
  });
});
