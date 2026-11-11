import type React from 'react';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import { useBoundSessionId } from '../contexts/TranscriptSessionContext';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { TranscriptSegment } from '../types/transcript';

export type ScrollDetachMode = 'streaming' | 'playback' | null;

export interface SegmentAnchor {
  segmentId: string;
  index: number;
  offsetFromTop: number;
}

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
  onUserInteraction: (e?: React.SyntheticEvent | Event) => void;
  scrollerRef: (ref: HTMLElement | null | Window) => void;
}

/**
 * Finds the first fully visible segment in the scroller viewport,
 * specifically excluding any segment that is partially clipped at the top.
 * If no segment is fully unclipped at the top (e.g. an oversized segment spanning
 * the viewport), falls back to the first segment intersecting the viewport.
 */
export function findFirstFullyVisibleSegment(
  scroller: HTMLElement,
  segments: TranscriptSegment[]
): SegmentAnchor | null {
  const scrollerRect = scroller.getBoundingClientRect();
  const segmentEls = scroller.querySelectorAll<HTMLElement>('.transcript-segment');
  let fallbackAnchor: SegmentAnchor | null = null;

  for (let i = 0; i < segmentEls.length; i++) {
    const el = segmentEls[i];
    const rect = el.getBoundingClientRect();

    // Check if segment intersects the visible scroller viewport
    const isIntersecting = rect.bottom > scrollerRect.top && rect.top < scrollerRect.bottom;
    if (!isIntersecting) {
      continue;
    }

    const segmentId = el.getAttribute('data-segment-id') || el.dataset.segmentId || '';
    const indexAttr = el.getAttribute('data-segment-index') || el.dataset.segmentIndex;
    let index =
      indexAttr !== undefined && indexAttr !== null && indexAttr !== ''
        ? parseInt(indexAttr, 10)
        : -1;
    if (index === -1 && segmentId) {
      index = segments.findIndex((s) => s.id === segmentId);
    }

    if (segmentId || index !== -1) {
      const anchor: SegmentAnchor = {
        segmentId,
        index,
        offsetFromTop: rect.top - scrollerRect.top,
      };

      // Segment top must not be clipped (rect.top >= scrollerRect.top - 1)
      if (rect.top >= scrollerRect.top - 1) {
        return anchor;
      }

      // If top is clipped, retain the first intersecting segment as fallback
      if (!fallbackAnchor) {
        fallbackAnchor = anchor;
      }
    }
  }
  return fallbackAnchor;
}

/**
 * Unified scroll follow hook for TranscriptEditor using Tri-Modal Segment Anchoring.
 *
 * Coordinates:
 * 1. Streaming follow (Real-time recording & batch transcription streaming):
 *    - Defaults to following latest output at the bottom (pinned to latest segment).
 *    - Detaches only when user deliberately scrolls away from bottom.
 *    - Re-engages when scrolling back to bottom or clicking resume button.
 * 2. Playback follow (Audio playback):
 *    - Follows currently playing segment (centered).
 *    - Preserves centered alignment when content above/below resizes.
 *    - Detaches when user scrolls away or enters editing mode.
 *    - Re-engages when clicking resume button or explicitly seeking.
 * 3. Free browsing (Reading mode):
 *    - Anchors to the first fully visible segment in the viewport.
 *    - Compensates scroll offset when content above changes height (e.g. translation appears),
 *      preventing reading layout jumps.
 */
export function useTranscriptScrollFollow({
  virtuosoRef,
  segments,
  editingSegmentId,
}: UseTranscriptScrollFollowOptions): UseTranscriptScrollFollowResult {
  const boundSessionId = useBoundSessionId();
  const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
  const isPaused = useTranscriptRuntimeStore((state) => state.isPaused);
  const recordingSessionId = useTranscriptStore((state) => state.recordingSessionId);
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);

  // Follow live streaming only when this editor is bound to the active recording session.
  const isCurrentSessionRecording =
    isRecording && (recordingSessionId ? boundSessionId === recordingSessionId : !sourceHistoryId);

  // Check if the current session is undergoing streaming batch transcription
  const isBatchStreaming = useBatchQueueStore((state) =>
    state.queueItems.some(
      (item) =>
        (item.id === boundSessionId || (sourceHistoryId && item.id === sourceHistoryId)) &&
        item.status === 'processing'
    )
  );

  const isStreamingActive = (isCurrentSessionRecording && !isPaused) || isBatchStreaming;

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

  // DOM Scroller and Anchoring state
  const [scrollerElement, setScrollerElement] = useState<HTMLElement | null>(null);
  const scrollerElementRef = useRef<HTMLElement | null>(null);
  const anchorRef = useRef<SegmentAnchor | null>(null);
  const isUserInteractingRef = useRef(false);
  const userInteractionTimerRef = useRef<ReturnType<typeof setTimeout>>(null);
  const isProgrammaticScrollRef = useRef(false);

  const scrollerRef = useCallback((element: HTMLElement | null | Window) => {
    const el = element instanceof HTMLElement ? element : null;
    scrollerElementRef.current = el;
    setScrollerElement(el);
    if (el && !anchorRef.current && !isStreamingActiveRef.current && !isPlayingRef.current) {
      anchorRef.current = findFirstFullyVisibleSegment(el, segmentsRef.current);
    }
  }, []);

  // When session switches, reset follow states to avoid inheriting detached state
  const prevSessionKeyRef = useRef(`${boundSessionId}:${sourceHistoryId}`);
  useEffect(() => {
    const currentKey = `${boundSessionId}:${sourceHistoryId}`;
    if (currentKey !== prevSessionKeyRef.current) {
      prevSessionKeyRef.current = currentKey;
      isStreamingFollowRef.current = true;
      setIsStreamingFollow(true);
      isPlaybackFollowRef.current = true;
      setIsPlaybackFollow(true);
      anchorRef.current = null;
      const currentSegments = segmentsRef.current;
      if (isStreamingActive && currentSegments.length > 0 && virtuosoRef.current) {
        virtuosoRef.current.scrollToIndex({
          index: currentSegments.length - 1,
          align: 'end',
          behavior: 'auto',
        });
      }
    }
  }, [boundSessionId, sourceHistoryId, isStreamingActive, virtuosoRef]);

  // When streaming becomes active, reset streaming follow to true
  const prevStreamingActiveRef = useRef(false);
  useEffect(() => {
    if (isStreamingActive && !prevStreamingActiveRef.current) {
      isStreamingFollowRef.current = true;
      setIsStreamingFollow(true);
      anchorRef.current = null;
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
        anchorRef.current = null;

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
      if (scrollerElementRef.current) {
        anchorRef.current = findFirstFullyVisibleSegment(
          scrollerElementRef.current,
          segmentsRef.current
        );
      }
    }
  }, [editingSegmentId]);

  // Content height and anchor compensation
  useLayoutEffect(() => {
    const scroller = scrollerElementRef.current;

    // 1. Streaming Follow Mode: keep pinned to bottom
    if (isStreamingActiveRef.current && isStreamingFollowRef.current && segments.length > 0) {
      virtuosoRef.current?.scrollToIndex({
        index: segments.length - 1,
        align: 'end',
        behavior: 'auto',
      });
      return;
    }

    // 2. Playback Follow Mode: keep playing segment centered
    if (
      isPlayingRef.current &&
      isPlaybackFollowRef.current &&
      !editingSegmentId &&
      activeSegmentIndexRef.current !== -1
    ) {
      virtuosoRef.current?.scrollToIndex({
        index: activeSegmentIndexRef.current,
        align: 'center',
        behavior: 'auto',
      });
      return;
    }

    // 3. Free Browsing Mode: Compensate scroll position to keep first fully visible segment stationary
    if (scroller && anchorRef.current) {
      const anchor = anchorRef.current;
      const anchorEl = scroller.querySelector<HTMLElement>(
        `[data-segment-id="${anchor.segmentId}"]`
      );
      if (anchorEl) {
        const scrollerRect = scroller.getBoundingClientRect();
        const currentRect = anchorEl.getBoundingClientRect();
        const currentOffsetFromTop = currentRect.top - scrollerRect.top;
        const delta = currentOffsetFromTop - anchor.offsetFromTop;
        if (Math.abs(delta) > 0.5) {
          isProgrammaticScrollRef.current = true;
          scroller.scrollTop += delta;
        }
      } else {
        // If previous anchor was deleted or unmounted, re-establish anchor
        anchorRef.current = findFirstFullyVisibleSegment(scroller, segmentsRef.current);
      }
    }
  }, [segments, editingSegmentId, virtuosoRef]);

  // Scroller event listeners for scrollbar tracking & user scroll detection
  useEffect(() => {
    if (!scrollerElement) return;

    const handlePointerDown = () => {
      isUserInteractingRef.current = true;
    };

    const handlePointerUp = () => {
      if (userInteractionTimerRef.current) {
        clearTimeout(userInteractionTimerRef.current);
      }
      userInteractionTimerRef.current = setTimeout(() => {
        isUserInteractingRef.current = false;
        if (
          !isStreamingFollowRef.current &&
          !isPlaybackFollowRef.current &&
          scrollerElementRef.current
        ) {
          anchorRef.current = findFirstFullyVisibleSegment(
            scrollerElementRef.current,
            segmentsRef.current
          );
        }
      }, 200);
    };

    const handleScroll = () => {
      if (isProgrammaticScrollRef.current) {
        isProgrammaticScrollRef.current = false;
        return;
      }
      if (isUserInteractingRef.current) {
        const isAtBottom =
          scrollerElement.scrollHeight -
            (scrollerElement.scrollTop + scrollerElement.clientHeight) <=
          60;
        if (isStreamingActiveRef.current && isAtBottom && !isStreamingFollowRef.current) {
          isStreamingFollowRef.current = true;
          setIsStreamingFollow(true);
          anchorRef.current = null;
        }
      }
    };

    scrollerElement.addEventListener('pointerdown', handlePointerDown);
    window.addEventListener('pointerup', handlePointerUp);
    scrollerElement.addEventListener('scroll', handleScroll, { passive: true });

    return () => {
      scrollerElement.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('pointerup', handlePointerUp);
      scrollerElement.removeEventListener('scroll', handleScroll);
      if (userInteractionTimerRef.current) {
        clearTimeout(userInteractionTimerRef.current);
      }
    };
  }, [scrollerElement]);

  // Virtuoso atBottom callback for streaming follow
  const atBottomStateChange = useCallback(
    (atBottom: boolean) => {
      if (isStreamingActiveRef.current) {
        if (atBottom) {
          isStreamingFollowRef.current = true;
          setIsStreamingFollow(true);
          anchorRef.current = null;
        } else {
          // If there is no real scroller element (e.g. headless unit tests),
          // or user is actively interacting (scrolling away), detach follow:
          if (!scrollerElementRef.current || isUserInteractingRef.current) {
            isStreamingFollowRef.current = false;
            setIsStreamingFollow(false);
            if (scrollerElementRef.current) {
              anchorRef.current = findFirstFullyVisibleSegment(
                scrollerElementRef.current,
                segmentsRef.current
              );
            }
          } else {
            // Content expansion caused non-bottom measurement:
            // Re-scroll to bottom to keep following
            if (segmentsRef.current.length > 0) {
              virtuosoRef.current?.scrollToIndex({
                index: segmentsRef.current.length - 1,
                align: 'end',
                behavior: 'auto',
              });
            }
          }
        }
      }
    },
    [virtuosoRef]
  );

  // User manual scroll detection for playback and streaming follow
  const onUserInteraction = useCallback((e?: React.SyntheticEvent | Event) => {
    isUserInteractingRef.current = true;
    if (userInteractionTimerRef.current) {
      clearTimeout(userInteractionTimerRef.current);
    }
    userInteractionTimerRef.current = setTimeout(() => {
      isUserInteractingRef.current = false;
      if (
        !isStreamingFollowRef.current &&
        !isPlaybackFollowRef.current &&
        scrollerElementRef.current
      ) {
        anchorRef.current = findFirstFullyVisibleSegment(
          scrollerElementRef.current,
          segmentsRef.current
        );
      }
    }, 200);

    if (isPlayingRef.current) {
      isPlaybackFollowRef.current = false;
      setIsPlaybackFollow(false);
    }

    if (isStreamingActiveRef.current && isStreamingFollowRef.current) {
      // If wheel scrolled upwards, detach streaming follow
      if (e && 'deltaY' in e && (e as React.WheelEvent).deltaY < 0) {
        isStreamingFollowRef.current = false;
        setIsStreamingFollow(false);
      }
    }

    // Immediately capture anchor if scroller element is mounted
    if (scrollerElementRef.current) {
      anchorRef.current = findFirstFullyVisibleSegment(
        scrollerElementRef.current,
        segmentsRef.current
      );
    }
  }, []);

  // Resume follow action
  const resumeFollow = useCallback(() => {
    if (isStreamingActiveRef.current) {
      isStreamingFollowRef.current = true;
      setIsStreamingFollow(true);
      anchorRef.current = null;
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
      anchorRef.current = null;
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
    scrollerRef,
  };
}
