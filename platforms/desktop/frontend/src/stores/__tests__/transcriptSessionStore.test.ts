import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import {
  getSessionStoreState,
  hasSessionContentChanged,
  resetSessionStoreCache,
  transcriptSessionStore,
  useTranscriptSessionStore,
} from '../transcriptSessionStore';
import { DEFAULT_SESSION_DATA, type SessionData, useTranscriptStore } from '../transcriptStore';

describe('transcriptSessionStore cache dirty checking and immutability', () => {
  beforeEach(() => {
    resetTranscriptStores();
    resetSessionStoreCache();
  });

  describe('hasSessionContentChanged', () => {
    it('returns false for identical objects or unchanged values', () => {
      const a: SessionData = { ...DEFAULT_SESSION_DATA };
      const b: SessionData = { ...DEFAULT_SESSION_DATA };
      expect(hasSessionContentChanged(a, a)).toBe(false);
      expect(hasSessionContentChanged(a, b)).toBe(false);
    });

    it('returns true when playback properties change', () => {
      const base: SessionData = { ...DEFAULT_SESSION_DATA };
      expect(hasSessionContentChanged(base, { ...base, currentTime: 2.5 })).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, isPlaying: true })).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, activeSegmentId: 'seg-1' })).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, activeSegmentIndex: 3 })).toBe(true);
      expect(
        hasSessionContentChanged(base, {
          ...base,
          seekRequest: { time: 10, timestamp: 12345 },
        })
      ).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, lastSeekTimestamp: 999 })).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, audioUrl: 'blob:test' })).toBe(true);
      expect(
        hasSessionContentChanged(base, {
          ...base,
          audioFile: new File([], 'test.wav'),
        })
      ).toBe(true);
    });

    it('returns true when session content properties change', () => {
      const base: SessionData = { ...DEFAULT_SESSION_DATA };
      expect(hasSessionContentChanged(base, { ...base, title: 'Updated' })).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, icon: 'wave' })).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, sourceHistoryId: 'h-1' })).toBe(true);
      expect(hasSessionContentChanged(base, { ...base, editingSegmentId: 'seg-e' })).toBe(true);
      expect(
        hasSessionContentChanged(base, {
          ...base,
          aligningSegmentIds: new Set(['seg-1']),
        })
      ).toBe(true);
    });
  });

  describe('getSessionStoreState immutability', () => {
    it('returns the same cached state when active session is completely unchanged', () => {
      const rawState = useTranscriptStore.getState();
      const direct1 = getSessionStoreState(rawState);
      const direct2 = getSessionStoreState(rawState);
      expect(direct1).toBe(direct2);

      const state1 = transcriptSessionStore.getState();
      const state2 = useTranscriptSessionStore.getState();
      expect(state1).toBe(state2);
      expect(state1).toBe(direct1);
    });

    it('returns a new reference on playback changes without mutating the previous snapshot', () => {
      const initialSnapshot = transcriptSessionStore.getState();
      expect(initialSnapshot.currentTime).toBe(0);
      expect(initialSnapshot.isPlaying).toBe(false);

      useTranscriptStore.getState().setCurrentTime(3.5);
      const afterTimeUpdate = transcriptSessionStore.getState();

      expect(afterTimeUpdate).not.toBe(initialSnapshot);
      expect(initialSnapshot.currentTime).toBe(0);
      expect(afterTimeUpdate.currentTime).toBe(3.5);

      useTranscriptStore.getState().setIsPlaying(true);
      const afterPlayUpdate = transcriptSessionStore.getState();

      expect(afterPlayUpdate).not.toBe(afterTimeUpdate);
      expect(afterTimeUpdate.isPlaying).toBe(false);
      expect(afterPlayUpdate.isPlaying).toBe(true);
    });
  });

  describe('transcriptSessionStore.subscribe', () => {
    it('notifies subscribers on playback changes', () => {
      const listener = vi.fn();
      const unsubscribe = transcriptSessionStore.subscribe(listener);

      useTranscriptStore.getState().setCurrentTime(7.2);
      expect(listener).toHaveBeenCalledTimes(1);
      expect(listener).toHaveBeenCalledWith(
        expect.objectContaining({ currentTime: 7.2 }),
        expect.objectContaining({ currentTime: 0 })
      );

      useTranscriptStore.getState().setIsPlaying(true);
      expect(listener).toHaveBeenCalledTimes(2);
      expect(listener).toHaveBeenCalledWith(
        expect.objectContaining({ isPlaying: true }),
        expect.objectContaining({ isPlaying: false })
      );

      unsubscribe();
    });
  });
});
