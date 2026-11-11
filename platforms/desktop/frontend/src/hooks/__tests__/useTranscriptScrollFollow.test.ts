import { act, renderHook } from '@testing-library/react';
import type React from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { DEFAULT_SESSION_DATA } from '../../stores/transcriptStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import type { TranscriptSegment } from '../../types/transcript';
import {
  findFirstFullyVisibleSegment,
  useTranscriptScrollFollow,
} from '../useTranscriptScrollFollow';

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
      recordingSessionId: null,
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
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
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
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
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

    it('follows when the last segment translation or finalization state changes', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
      });

      const streamingSegments: TranscriptSegment[] = [
        ...initialSegments,
        {
          id: 'seg-3',
          start: 2,
          end: 3,
          text: 'Interim text',
          isFinal: false,
          tokens: [],
          timestamps: [],
        },
      ];

      const { rerender } = renderHook(
        ({ segments }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments,
          }),
        { initialProps: { segments: streamingSegments } }
      );

      mockScrollToIndex.mockClear();

      // Interim text receives translation
      const translatedSegments = [
        initialSegments[0],
        initialSegments[1],
        {
          ...streamingSegments[2],
          translation: 'Translated interim',
        },
      ];
      rerender({ segments: translatedSegments });

      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 2,
        align: 'end',
        behavior: 'auto',
      });

      mockScrollToIndex.mockClear();

      // Segment finalizes
      const finalizedSegments = [
        initialSegments[0],
        initialSegments[1],
        {
          ...translatedSegments[2],
          isFinal: true,
        },
      ];
      rerender({ segments: finalizedSegments });

      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 2,
        align: 'end',
        behavior: 'auto',
      });
    });

    it('detaches follow when user scrolls away from bottom and stops auto-scrolling', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
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
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
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
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
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
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
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

    it('does not treat background live recording as streaming for an inspected completed record', () => {
      act(() => {
        useTranscriptStore.setState({
          activeSessionId: 'completed-1',
          recordingSessionId: 'rec-live-1',
          sourceHistoryId: 'completed-1',
          isRecording: true,
          isPaused: false,
        });
      });

      const { result, rerender } = renderHook(
        ({ segments }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments,
          }),
        { initialProps: { segments: initialSegments } }
      );

      expect(result.current.isStreamingActive).toBe(false);
      mockScrollToIndex.mockClear();

      rerender({
        segments: [
          ...initialSegments,
          {
            id: 'seg-3',
            start: 2,
            end: 3,
            text: 'Appended',
            isFinal: true,
            tokens: [],
            timestamps: [],
          },
        ],
      });

      expect(mockScrollToIndex).not.toHaveBeenCalled();
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

  describe('Tri-Modal Segment Anchoring & Translation Anti-Jump', () => {
    it('follows when an earlier (non-last) segment receives translation during streaming', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
      });

      const streamingSegments: TranscriptSegment[] = [
        { id: 'seg-1', start: 0, end: 1, text: 'Hello', isFinal: true, tokens: [], timestamps: [] },
        { id: 'seg-2', start: 1, end: 2, text: 'World', isFinal: true, tokens: [], timestamps: [] },
        {
          id: 'seg-3',
          start: 2,
          end: 3,
          text: 'Streaming...',
          isFinal: false,
          tokens: [],
          timestamps: [],
        },
      ];

      const { rerender } = renderHook(
        ({ segments }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments,
          }),
        { initialProps: { segments: streamingSegments } }
      );

      mockScrollToIndex.mockClear();

      // seg-1 (an earlier segment) receives translation
      const updatedSegments: TranscriptSegment[] = [
        { ...streamingSegments[0], translation: '你好' },
        streamingSegments[1],
        streamingSegments[2],
      ];

      rerender({ segments: updatedSegments });

      // Must follow to bottom (index 2) so that content expansion does not detach follow
      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 2,
        align: 'end',
        behavior: 'auto',
      });
    });

    it('does not detach streaming follow when atBottomStateChange(false) fires from layout expansion without user interaction', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
      });

      // Create a mock scroller element
      const mockScroller = document.createElement('div');
      Object.defineProperty(mockScroller, 'scrollHeight', { value: 1000, configurable: true });
      Object.defineProperty(mockScroller, 'clientHeight', { value: 500, configurable: true });
      Object.defineProperty(mockScroller, 'scrollTop', {
        value: 450,
        writable: true,
        configurable: true,
      });

      const { result } = renderHook(() =>
        useTranscriptScrollFollow({
          virtuosoRef: mockVirtuosoRef,
          segments: initialSegments,
        })
      );

      // Connect scroller ref
      act(() => {
        result.current.scrollerRef(mockScroller);
      });

      mockScrollToIndex.mockClear();

      // Virtuoso signals atBottomStateChange(false) due to translation expansion, but user is NOT scrolling
      act(() => {
        result.current.atBottomStateChange(false);
      });

      // Should NOT detach because user did not scroll!
      expect(result.current.detachMode).toBeNull();
      // Should re-scroll to bottom to keep anchored at the bottom
      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: initialSegments.length - 1,
        align: 'end',
        behavior: 'auto',
      });
    });

    it('detaches streaming follow when user scrolls with wheel upwards', () => {
      act(() => {
        useTranscriptStore.setState({ isRecording: true, recordingSessionId: 'session-1' });
      });

      const { result } = renderHook(() =>
        useTranscriptScrollFollow({
          virtuosoRef: mockVirtuosoRef,
          segments: initialSegments,
        })
      );

      // User scrolls upwards with mouse wheel (deltaY < 0)
      act(() => {
        result.current.onUserInteraction({ deltaY: -100 } as unknown as React.WheelEvent);
      });

      expect(result.current.detachMode).toBe('streaming');
    });

    it('re-aligns active segment centered during completed transcript playback when a segment receives translation', () => {
      act(() => {
        useTranscriptStore.setState({
          isPlaying: true,
          activeSegmentIndex: 1,
        });
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

      // A segment receives translation while audio is playing
      const translatedSegments: TranscriptSegment[] = [
        { ...initialSegments[0], translation: '你好' },
        initialSegments[1],
      ];

      rerender({ segments: translatedSegments });

      // Must re-center active segment so it stays steady without jumping
      expect(mockScrollToIndex).toHaveBeenCalledWith({
        index: 1,
        align: 'center',
        behavior: 'auto',
      });
    });

    describe('findFirstFullyVisibleSegment', () => {
      it('skips segments clipped at the top and selects the first fully visible segment', () => {
        const scroller = document.createElement('div');
        vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue({
          top: 100,
          bottom: 500,
          left: 0,
          right: 300,
          width: 300,
          height: 400,
          x: 0,
          y: 100,
          toJSON: () => {},
        });

        // Segment 0: partially clipped at top (top = 80, scroller top = 100)
        const seg0 = document.createElement('div');
        seg0.className = 'transcript-segment';
        seg0.setAttribute('data-segment-id', 'seg-0');
        seg0.setAttribute('data-segment-index', '0');
        vi.spyOn(seg0, 'getBoundingClientRect').mockReturnValue({
          top: 80,
          bottom: 120,
          left: 0,
          right: 300,
          width: 300,
          height: 40,
          x: 0,
          y: 80,
          toJSON: () => {},
        });
        scroller.appendChild(seg0);

        // Segment 1: fully visible at top (top = 120, >= scroller top 100)
        const seg1 = document.createElement('div');
        seg1.className = 'transcript-segment';
        seg1.setAttribute('data-segment-id', 'seg-1');
        seg1.setAttribute('data-segment-index', '1');
        vi.spyOn(seg1, 'getBoundingClientRect').mockReturnValue({
          top: 120,
          bottom: 170,
          left: 0,
          right: 300,
          width: 300,
          height: 50,
          x: 0,
          y: 120,
          toJSON: () => {},
        });
        scroller.appendChild(seg1);

        const segments: TranscriptSegment[] = [
          {
            id: 'seg-0',
            start: 0,
            end: 1,
            text: 'Top clipped',
            isFinal: true,
            tokens: [],
            timestamps: [],
          },
          {
            id: 'seg-1',
            start: 1,
            end: 2,
            text: 'First fully visible',
            isFinal: true,
            tokens: [],
            timestamps: [],
          },
        ];

        const anchor = findFirstFullyVisibleSegment(scroller, segments);

        expect(anchor).not.toBeNull();
        expect(anchor?.segmentId).toBe('seg-1');
        expect(anchor?.index).toBe(1);
        expect(anchor?.offsetFromTop).toBe(20); // 120 - 100
      });
    });

    it('compensates scroll position in free browsing mode when an upper segment expands in height', () => {
      const mockScroller = document.createElement('div');
      Object.defineProperty(mockScroller, 'scrollTop', {
        value: 200,
        writable: true,
        configurable: true,
      });
      vi.spyOn(mockScroller, 'getBoundingClientRect').mockReturnValue({
        top: 100,
        bottom: 500,
        left: 0,
        right: 300,
        width: 300,
        height: 400,
        x: 0,
        y: 100,
        toJSON: () => {},
      });

      const seg1 = document.createElement('div');
      seg1.className = 'transcript-segment';
      seg1.setAttribute('data-segment-id', 'seg-1');
      seg1.setAttribute('data-segment-index', '1');
      // Initially at top = 150 (offsetFromTop = 50)
      const seg1RectSpy = vi.spyOn(seg1, 'getBoundingClientRect').mockReturnValue({
        top: 150,
        bottom: 200,
        left: 0,
        right: 300,
        width: 300,
        height: 50,
        x: 0,
        y: 150,
        toJSON: () => {},
      });
      mockScroller.appendChild(seg1);

      const segments: TranscriptSegment[] = [
        { id: 'seg-0', start: 0, end: 1, text: 'Seg 0', isFinal: true, tokens: [], timestamps: [] },
        { id: 'seg-1', start: 1, end: 2, text: 'Seg 1', isFinal: true, tokens: [], timestamps: [] },
      ];

      const { result, rerender } = renderHook(
        ({ segs }) =>
          useTranscriptScrollFollow({
            virtuosoRef: mockVirtuosoRef,
            segments: segs,
          }),
        { initialProps: { segs: segments } }
      );

      act(() => {
        result.current.scrollerRef(mockScroller);
      });

      // User scrolls away, establishing seg-1 as anchor
      act(() => {
        result.current.onUserInteraction();
      });

      // Now seg-0 (above seg-1) receives a translation (+40px), pushing seg-1 down to top = 190
      seg1RectSpy.mockReturnValue({
        top: 190,
        bottom: 240,
        left: 0,
        right: 300,
        width: 300,
        height: 50,
        x: 0,
        y: 190,
        toJSON: () => {},
      });

      const updatedSegments: TranscriptSegment[] = [
        { ...segments[0], translation: 'Seg 0 translated' },
        segments[1],
      ];

      rerender({ segs: updatedSegments });

      // scrollTop should be compensated by delta (190 - 150 = 40): 200 + 40 = 240
      expect(mockScroller.scrollTop).toBe(240);
    });
  });
});
