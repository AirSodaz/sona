import type { TranscriptSegment } from '../types/transcript';

export interface TranscriptActiveSessionState {
  segments: TranscriptSegment[];
  activeSegmentId: string | null;
  activeSegmentIndex: number;
  editingSegmentId: string | null;
  aligningSegmentIds: Set<string>;
  audioFile: File | null;
  audioUrl: string | null;
  currentTime: number;
  isPlaying: boolean;
  lastSeekTimestamp: number;
  seekRequest: { time: number; timestamp: number } | null;
  sourceHistoryId: string | null;
  title: string | null;
  icon: string | null;
}

export type TranscriptSessionStateFields = Pick<
  TranscriptActiveSessionState,
  'segments' | 'editingSegmentId' | 'aligningSegmentIds' | 'sourceHistoryId' | 'title' | 'icon'
>;

export type TranscriptPlaybackStateFields = Pick<
  TranscriptActiveSessionState,
  'activeSegmentId' | 'activeSegmentIndex' | 'audioFile' | 'audioUrl' | 'currentTime' | 'isPlaying' | 'lastSeekTimestamp' | 'seekRequest'
>;

export const INITIAL_TRANSCRIPT_ACTIVE_SESSION_STATE: TranscriptActiveSessionState = {
  segments: [],
  activeSegmentId: null,
  activeSegmentIndex: -1,
  editingSegmentId: null,
  aligningSegmentIds: new Set<string>(),
  audioFile: null,
  audioUrl: null,
  currentTime: 0,
  isPlaying: false,
  lastSeekTimestamp: 0,
  seekRequest: null,
  sourceHistoryId: null,
  title: null,
  icon: null,
};

export const INITIAL_TRANSCRIPT_SESSION_STATE: TranscriptSessionStateFields = {
  segments: [],
  editingSegmentId: null,
  aligningSegmentIds: new Set<string>(),
  sourceHistoryId: null,
  title: null,
  icon: null,
};

export const INITIAL_TRANSCRIPT_PLAYBACK_STATE: TranscriptPlaybackStateFields = {
  activeSegmentId: null,
  activeSegmentIndex: -1,
  audioFile: null,
  audioUrl: null,
  currentTime: 0,
  isPlaying: false,
  lastSeekTimestamp: 0,
  seekRequest: null,
};
