import type React from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { TranscriptSegment } from '../types/transcript';

export type ScrollDetachMode = 'streaming' | 'playback' | null;

export interface UseTranscriptScrollFollowOptions {
  virtuosoRef: React.RefObject<VirtuosoHandle | null>;
  segments: TranscriptSegment[];
  editingSegmentId?: string | null;
}

export interface UseTranscriptScrollFollowResult {
  detachMode: ScrollDetachMode;
  isStreamingActive: boolean;
  isPlaying: boolean;
  resumeFollow: () => void;
  atBottomStateChange: (atBottom: boolean) => void;
  onUserInteraction: () => void;
}

/**
 * Unified scroll follow hook for TranscriptEditor.
 *
 * Coordinates:
 * 1. Streaming follow (Real-time recording & batch transcription streaming):
 *    - Defaults to following latest output at the bottom.
 *    - Detaches when user scrolls away from bottom.
 *    - Re-engages when scrolling back to bottom or clicking resume button.
 * 2. Playback follow (Audio playback):
 *    - Follows currently playing segment (centered).
 *    - Detaches when user scrolls away or enters editing mode.
 *    - Re-engages when clicking resume button or explicitly seeking.
 */
export function useTranscriptScrollFollow({
  virtuosoRef,
  segments,
  editingSegmentId,
}: UseTranscriptScrollFollowOptions): UseTranscriptScrollFollowResult {
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const isPaused = useTranscriptRuntimeStore((state) => state.isPaused);
  const activeSessionId = useTranscriptStore((state) => state.activeSessionId);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);

  // Check if the current session is undergoing streaming batch transcription
  const isBatchStreaming = useBatchQueueStore((state) =>
    state.queueItems.some(
      (item) =>
        (item.id === activeSessionId || (sourceHistoryId && item.id === sourceHistoryId)) &&
        item.status === 'processing'
    )
  );

  const isStreamingActive = (isRecording && !isPaused) || isBatchStreaming;

  // Playback state
  const isPlaying = useTranscriptPlaybackStore((state) => state.isPlaying);
  const activeSegmentIndex = useTranscriptPlaybackStore((state) => state.activeSegmentIndex);
  const lastSeekTimestamp = useTranscriptPlaybackStore((state) => state.lastSeekTimestamp);

  // Follow states
  const [isStreamingFollow, setIsStreamingFollow] = useState(true);
  const [isPlaybackFollow, setIsPlaybackFollow] = useState(true);

  const isStreamingFollowRef = useRef(true);
  const isPlaybackFollowRef = useRef(true);
  const isStreamingActiveRef = useRef(isStreamingActive);
  const isPlayingRef = useRef(isPlaying);
  const activeSegmentIndexRef = useRef(activeSegmentIndex);
  const segmentsRef = useRef(segments);

  isStreamingFollowRef.current = isStreamingFollow;
  isPlaybackFollowRef.current = isPlaybackFollow;
  isStreamingActiveRef.current = isStreamingActive;
  isPlayingRef.current = isPlaying;
  activeSegmentIndexRef.current = activeSegmentIndex;
  segmentsRef.current = segments;

  // When session switches, reset follow states to avoid inheriting detached state
  const prevSessionKeyRef = useRef(`${activeSessionId}:${sourceHistoryId}`);
  useEffect(() => {
    const currentKey = `${activeSessionId}:${sourceHistoryId}`;
    if (currentKey !== prevSessionKeyRef.current) {
      prevSessionKeyRef.current = currentKey;
      isStreamingFollowRef.current = true;
      setIsStreamingFollow(true);
      isPlaybackFollowRef.current = true;
      setIsPlaybackFollow(true);
      const currentSegments = segmentsRef.current;
      if (isStreamingActive && currentSegments.length > 0 && virtuosoRef.current) {
        virtuosoRef.current.scrollToIndex({
          index: currentSegments.length - 1,
          align: 'end',
          behavior: 'auto',
        });
      }
    }
  }, [activeSessionId, sourceHistoryId, isStreamingActive, virtuosoRef]);

  // When streaming becomes active, reset streaming follow to true
  const prevStreamingActiveRef = useRef(false);
  useEffect(() => {
    if (isStreamingActive && !prevStreamingActiveRef.current) {
      isStreamingFollowRef.current = true;
      setIsStreamingFollow(true);
      const currentSegments = segmentsRef.current;
      if (currentSegments.length > 0 && virtuosoRef.current) {
        virtuosoRef.current.scrollToIndex({
          index: currentSegments.length - 1,
          align: 'end',
          behavior: 'auto',
        });
      }
    }
    prevStreamingActiveRef.current = isStreamingActive;
  }, [isStreamingActive, virtuosoRef]);

  // When audio playback starts or user seeks, reset playback follow to true (unless editing)
  const prevPlayingRef = useRef(false);
  const prevSeekRef = useRef(lastSeekTimestamp);

  useEffect(() => {
    const playbackStarted = isPlaying && !prevPlayingRef.current;
    const userSought = lastSeekTimestamp !== prevSeekRef.current;

    prevPlayingRef.current = isPlaying;
    prevSeekRef.current = lastSeekTimestamp;

    if (playbackStarted || userSought) {
      if (!editingSegmentId) {
        isPlaybackFollowRef.current = true;
        setIsPlaybackFollow(true);

        if (activeSegmentIndex !== -1 && virtuosoRef.current) {
          virtuosoRef.current.scrollToIndex({
            index: activeSegmentIndex,
            align: 'center',
            behavior: 'smooth',
          });
        }
      } else {
        isPlaybackFollowRef.current = false;
        setIsPlaybackFollow(false);
      }
    }
  }, [isPlaying, lastSeekTimestamp, activeSegmentIndex, editingSegmentId, virtuosoRef]);

  // When activeSegmentIndex changes during playback, follow if enabled and not editing
  const prevActiveSegmentIndexRef = useRef(activeSegmentIndex);
  useEffect(() => {
    const indexChanged = activeSegmentIndex !== prevActiveSegmentIndexRef.current;
    prevActiveSegmentIndexRef.current = activeSegmentIndex;

    if (
      indexChanged &&
      isPlaying &&
      isPlaybackFollowRef.current &&
      !editingSegmentId &&
      activeSegmentIndex !== -1 &&
      virtuosoRef.current
    ) {
      virtuosoRef.current.scrollToIndex({
        index: activeSegmentIndex,
        align: 'center',
        behavior: 'smooth',
      });
    }
  }, [activeSegmentIndex, isPlaying, editingSegmentId, virtuosoRef]);

  // When entering editing mode during playback, detach playback follow
  useEffect(() => {
    if (editingSegmentId && isPlayingRef.current) {
      isPlaybackFollowRef.current = false;
      setIsPlaybackFollow(false);
    }
  }, [editingSegmentId]);

  // Follow latest output when streaming and new segments arrive or segment content changes
  const lastSegment = segments[segments.length - 1];
  const lastSegmentKey = lastSegment
    ? `${lastSegment.id}:${lastSegment.text}:${lastSegment.translation ?? ''}:${lastSegment.isFinal ? 1 : 0}`
    : '';
  useEffect(() => {
    void lastSegmentKey;
    if (isStreamingActiveRef.current && isStreamingFollowRef.current && segments.length > 0) {
      virtuosoRef.current?.scrollToIndex({
        index: segments.length - 1,
        align: 'end',
        behavior: 'auto',
      });
    }
  }, [segments.length, lastSegmentKey, virtuosoRef]);

  // Virtuoso atBottom callback for streaming follow
  const atBottomStateChange = useCallback((atBottom: boolean) => {
    if (isStreamingActiveRef.current) {
      isStreamingFollowRef.current = atBottom;
      setIsStreamingFollow(atBottom);
    }
  }, []);

  // User manual scroll detection for playback follow
  const onUserInteraction = useCallback(() => {
    if (isPlayingRef.current) {
      isPlaybackFollowRef.current = false;
      setIsPlaybackFollow(false);
    }
  }, []);

  // Resume follow action
  const resumeFollow = useCallback(() => {
    if (isStreamingActiveRef.current) {
      isStreamingFollowRef.current = true;
      setIsStreamingFollow(true);
      const segs = segmentsRef.current;
      if (segs.length > 0 && virtuosoRef.current) {
        virtuosoRef.current.scrollToIndex({
          index: segs.length - 1,
          align: 'end',
          behavior: 'smooth',
        });
      }
    } else if (isPlayingRef.current) {
      isPlaybackFollowRef.current = true;
      setIsPlaybackFollow(true);
      const idx = activeSegmentIndexRef.current;
      if (idx !== -1 && virtuosoRef.current) {
        virtuosoRef.current.scrollToIndex({
          index: idx,
          align: 'center',
          behavior: 'smooth',
        });
      }
    }
  }, [virtuosoRef]);

  // Calculate current detach mode
  let detachMode: ScrollDetachMode = null;
  if (isStreamingActive && !isStreamingFollow) {
    detachMode = 'streaming';
  } else if (isPlaying && !isPlaybackFollow) {
    detachMode = 'playback';
  }

  return {
    detachMode,
    isStreamingActive,
    isPlaying,
    resumeFollow,
    atBottomStateChange,
    onUserInteraction,
  };
}
