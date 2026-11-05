import { act, renderHook } from '@testing-library/react';
import type React from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { DEFAULT_SESSION_DATA } from '../../stores/transcriptStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import type { TranscriptSegment } from '../../types/transcript';
import { useTranscriptScrollFollow } from '../useTranscriptScrollFollow';

describe('useTranscriptScrollFollow', () => {
  let mockScrollToIndex: ReturnType<typeof vi.fn>;
  let mockVirtuosoRef: React.RefObject<VirtuosoHandle | null>;

  const initialSegments: TranscriptSegment[] = [
    { id: 'seg-1', start: 0, end: 1, text: 'Hello', isFinal: true, tokens: [], timestamps: [] },
    { id: 'seg-2', start: 1, end: 2, text: 'World', isFinal: true, tokens: [], timestamps: [] },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    mockScrollToIndex = vi.fn();
    mockVirtuosoRef = {
      current: {
        scrollToIndex: mockScrollToIndex,
      } as unknown as VirtuosoHandle,
    };

    useTranscriptStore.setState({
      activeSessionId: 'session-1',
      sourceHistoryId: 'session-1',
      isRecording: false,
      isPaused: false,
      isPlaying: false,
      activeSegmentIndex: -1,
      lastSeekTimestamp: 0,
      sessions: {
        'session-1': {
          ...DEFAULT_SESSION_DATA,
          segments: initialSegments,
        },
      },
    });

    useBatchQueueStore.setState({
      queueItems: [],
      activeItemId: null,
      isQueueProcessing: false,
      isQueuePaused: false,
    });
  });

  describe('Streaming Output Follow', () => {
    it('automatically scrolls to the bottom when recording starts', () => {
      const { result, rerender } = renderHook(
        ({ segments }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments,
          }),
        { initialProps: { segments: initialSegments } }
      );

      expect(result.current.detachMode).toBeNull();

      act(() => {
        useTranscriptStore.setState({ isRecording: true });
      });
      rerender({ segments: initialSegments });

      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: initialSegments.length - 1,
        align: 'end',
        behavior: 'auto',
      });
    });

    it('follows newly added segments when streaming', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true });
      });

      const { rerender } = renderHook(
        ({ segments }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments,
          }),
        { initialProps: { segments: initialSegments } }
      );

      mockScrollToIndex.mockClear();

      const updatedSegments = [
        ...initialSegments,
        {
          id: 'seg-3',
          start: 2,
          end: 3,
          text: 'New Line',
          isFinal: false,
          tokens: [],
          timestamps: [],
        },
      ];

      rerender({ segments: updatedSegments });

      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 2,
        align: 'end',
        behavior: 'auto',
      });
    });

    it('detaches follow when user scrolls away from bottom and stops auto-scrolling', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true });
      });

      const { result, rerender } = renderHook(
        ({ segments }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments,
          }),
        { initialProps: { segments: initialSegments } }
      );

      // User scrolls up (atBottom becomes false)
      act(() => {
        result.current.atBottomStateChange(false);
      });

      expect(result.current.detachMode).toBe('streaming');

      mockScrollToIndex.mockClear();

      // New segment arrives while detached
      const updatedSegments = [
        ...initialSegments,
        {
          id: 'seg-3',
          start: 2,
          end: 3,
          text: 'Another',
          isFinal: false,
          tokens: [],
          timestamps: [],
        },
      ];
      rerender({ segments: updatedSegments });

      // Should NOT auto-scroll because user detached
      expect(mockScrollToIndex).not.toHaveBeenCalled();
    });

    it('re-engages follow and smoothly scrolls to bottom when resumeFollow is called', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true });
      });

      const { result } = renderHook(() =>
        useTranscriptScrollFollow({
          virtuosoRef: mockVirtuosoRef,
          segments: initialSegments,
        })
      );

      act(() => {
        result.current.atBottomStateChange(false);
      });
      expect(result.current.detachMode).toBe('streaming');

      mockScrollToIndex.mockClear();

      act(() => {
        result.current.resumeFollow();
      });

      expect(result.current.detachMode).toBeNull();
      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: initialSegments.length - 1,
        align: 'end',
        behavior: 'smooth',
      });
    });

    it('re-engages follow when user scrolls back to bottom', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true });
      });

      const { result } = renderHook(() =>
        useTranscriptScrollFollow({
          virtuosoRef: mockVirtuosoRef,
          segments: initialSegments,
        })
      );

      act(() => {
        result.current.atBottomStateChange(false);
      });
      expect(result.current.detachMode).toBe('streaming');

      act(() => {
        result.current.atBottomStateChange(true);
      });
      expect(result.current.detachMode).toBeNull();
    });

    it('supports batch streaming output follow for the active batch item', () => {
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'session-1',
            filePath: 'test.mp3',
            filename: 'test.mp3',
            status: 'processing',
            progress: 50,
            segments: [],
            projectId: null,
          },
        ],
      });

      const { result, rerender } = renderHook(
        ({ segments }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments,
          }),
        { initialProps: { segments: initialSegments } }
      );

      expect(result.current.isStreamingActive).toBe(true);

      mockScrollToIndex.mockClear();

      const nextSegments = [
        ...initialSegments,
        {
          id: 'seg-batch',
          start: 2,
          end: 3,
          text: 'Batch text',
          isFinal: true,
          tokens: [],
          timestamps: [],
        },
      ];
      rerender({ segments: nextSegments });

      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 2,
        align: 'end',
        behavior: 'auto',
      });
    });

    it('resets follow state when switching active session during streaming', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true });
      });

      const { result, rerender } = renderHook(() =>
        useTranscriptScrollFollow({
          virtuosoRef: mockVirtuosoRef,
          segments: initialSegments,
        })
      );

      // User detaches in session 1
      act(() => {
        result.current.atBottomStateChange(false);
      });
      expect(result.current.detachMode).toBe('streaming');

      // User switches to session 2
      act(() => {
        useTranscriptStore.setState({ activeSessionId: 'session-2', sourceHistoryId: 'session-2' });
      });
      rerender();

      // Follow should be cleanly reset to true (detachMode is null)
      expect(result.current.detachMode).toBeNull();
    });
  });

  describe('Audio Playback Follow', () => {
    it('centers active segment when audio playback starts', () => {
      renderHook(() =>
        useTranscriptScrollFollow({
          virtuosoRef: mockVirtuosoRef,
          segments: initialSegments,
        })
      );

      mockScrollToIndex.mockClear();

      act(() => {
        useTranscriptStore.setState({
          isPlaying: true,
          activeSegmentIndex: 1,
        });
      });

      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 1,
        align: 'center',
        behavior: 'smooth',
      });
    });

    it('detaches playback follow when user interacts (scrolls)', () => {
      act(() => {
        useTranscriptStore.setState({
          isPlaying: true,
          activeSegmentIndex: 0,
        });
      });

      const { result } = renderHook(() =>
        useTranscriptScrollFollow({
          virtuosoRef: mockVirtuosoRef,
          segments: initialSegments,
        })
      );

      // User scrolls while playing
      act(() => {
        result.current.onUserInteraction();
      });

      expect(result.current.detachMode).toBe('playback');

      mockScrollToIndex.mockClear();

      // Next segment begins playing while detached
      act(() => {
        useTranscriptStore.setState({ activeSegmentIndex: 1 });
      });

      // Does not snatch scroll position
      expect(mockScrollToIndex).not.toHaveBeenCalled();

      // Clicking resume re-centers active segment
      act(() => {
        result.current.resumeFollow();
      });

      expect(result.current.detachMode).toBeNull();
      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 1,
        align: 'center',
        behavior: 'smooth',
      });
    });

    it('pauses playback follow when entering editing mode', () => {
      act(() => {
        useTranscriptStore.setState({
          isPlaying: true,
          activeSegmentIndex: 0,
        });
      });

      const { result, rerender } = renderHook(
        ({ editingSegmentId }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments: initialSegments,
            editingSegmentId,
          }),
        { initialProps: { editingSegmentId: null as string | null } }
      );

      // Enter editing mode
      rerender({ editingSegmentId: 'seg-1' });

      expect(result.current.detachMode).toBe('playback');

      mockScrollToIndex.mockClear();

      // Playback advances to next segment while editing
      act(() => {
        useTranscriptStore.setState({ activeSegmentIndex: 1 });
      });

      expect(mockScrollToIndex).not.toHaveBeenCalled();
    });
  });
});
