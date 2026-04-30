import { create } from 'zustand';
import { findSegmentAndIndexForTime } from '../utils/segmentUtils';
import { useTranscriptSessionStore } from './transcriptSessionStore';
import {
  INITIAL_TRANSCRIPT_PLAYBACK_STATE,
  type TranscriptPlaybackStateFields,
} from './transcriptSessionState';

export interface TranscriptPlaybackState extends TranscriptPlaybackStateFields {
  setAudioFile: (file: File | null) => void;
  setAudioUrl: (url: string | null) => void;
  setCurrentTime: (time: number) => void;
  setIsPlaying: (isPlaying: boolean) => void;
  setActiveSegmentId: (id: string | null, index?: number) => void;
  resetActiveSegmentIndex: () => void;
  requestSeek: (time: number) => void;
  openSession: (audioUrl?: string | null) => void;
  clearSession: (options?: { clearAudio?: boolean }) => void;
}

export const useTranscriptPlaybackStore = create<TranscriptPlaybackState>((set, get) => ({
  ...INITIAL_TRANSCRIPT_PLAYBACK_STATE,

  setAudioFile: (file) => {
    const state = get();
    if (state.audioUrl) {
      URL.revokeObjectURL(state.audioUrl);
    }

    const url = file ? URL.createObjectURL(file) : null;
    set({
      audioFile: file,
      audioUrl: url,
      isPlaying: false,
      currentTime: 0,
      activeSegmentId: null,
      activeSegmentIndex: -1,
      seekRequest: null,
      lastSeekTimestamp: 0,
    });
  },

  setAudioUrl: (audioUrl) => set({
    audioUrl,
    isPlaying: false,
    currentTime: 0,
    activeSegmentId: null,
    activeSegmentIndex: -1,
    seekRequest: null,
    lastSeekTimestamp: 0,
  }),

  setCurrentTime: (time) => {
    const state = get();
    const segments = useTranscriptSessionStore.getState().segments;
    const { segment, index } = findSegmentAndIndexForTime(
      segments,
      time,
      state.activeSegmentIndex,
    );

    if (segment?.id !== state.activeSegmentId) {
      set({
        currentTime: time,
        activeSegmentId: segment?.id || null,
        activeSegmentIndex: index,
      });
      return;
    }

    set({ currentTime: time });
  },

  setIsPlaying: (isPlaying) => set({ isPlaying }),

  setActiveSegmentId: (activeSegmentId, activeSegmentIndex = -1) => set({
    activeSegmentId,
    activeSegmentIndex,
  }),

  resetActiveSegmentIndex: () => set({ activeSegmentIndex: -1 }),

  requestSeek: (time) => {
    get().setCurrentTime(time);
    const timestamp = Date.now();
    set({
      seekRequest: { time, timestamp },
      lastSeekTimestamp: timestamp,
    });
  },

  openSession: (audioUrl) => set((state) => ({
    ...INITIAL_TRANSCRIPT_PLAYBACK_STATE,
    audioFile: state.audioFile,
    audioUrl: audioUrl !== undefined ? audioUrl : state.audioUrl,
  })),

  clearSession: (options) => set((state) => ({
    ...INITIAL_TRANSCRIPT_PLAYBACK_STATE,
    audioFile: options?.clearAudio ? null : state.audioFile,
    audioUrl: options?.clearAudio ? null : state.audioUrl,
  })),
}));
