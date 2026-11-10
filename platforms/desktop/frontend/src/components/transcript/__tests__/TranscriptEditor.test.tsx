import { act, fireEvent, render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG } from '../../../stores/configStore';
import { useSearchStore } from '../../../stores/searchStore';
import { useTranscriptStore } from '../../../test-utils/transcriptStoreTestUtils';
import TranscriptEditor from '../TranscriptEditor';

// Mock scrollTo
window.HTMLElement.prototype.scrollIntoView = vi.fn();

// Mock Virtuoso
const { mockScrollToIndex, segmentRenderCounts, lastVirtuosoProps } = vi.hoisted(() => ({
  mockScrollToIndex: vi.fn(),
  segmentRenderCounts: new Map<string, number>(),
  lastVirtuosoProps: { current: null as any },
}));

vi.mock('react-virtuoso', async () => {
  const React = await import('react');
  return {
    Virtuoso: React.forwardRef((props: any, ref: any) => {
      lastVirtuosoProps.current = props;
      React.useImperativeHandle(ref, () => ({
        scrollToIndex: mockScrollToIndex,
      }));
      const Header = props.components?.Header;
      const Footer = props.components?.Footer;
      return (
        <div data-testid="virtuoso-list">
          {Header && (
            <div data-testid="virtuoso-header">
              <Header />
            </div>
          )}
          {props.data?.map((item: any, index: number) =>
            props.itemContent(index, item, props.context)
          )}
          {Footer && (
            <div data-testid="virtuoso-footer">
              <Footer />
            </div>
          )}
        </div>
      );
    }),
  };
});

// Mock i18n
const t = (key: string) => key;
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

// Mock dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  ask: vi.fn(),
}));

// Mock SegmentItem for per-segment render tracking
vi.mock('../SegmentItem', async () => {
  const React = await import('react');
  const mockFn = vi.fn((props: { segment: { id: string } }) => {
    segmentRenderCounts.set(props.segment.id, (segmentRenderCounts.get(props.segment.id) ?? 0) + 1);
    return <div>Segment</div>;
  });
  const Memoized = React.memo(mockFn);
  return {
    SegmentItem: Memoized,
  };
});

describe('TranscriptEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    act(() => {
      useTranscriptStore.setState({
        segments: [],
        activeSegmentId: null,
        activeSegmentIndex: -1,
        isPlaying: false,
        editingSegmentId: null,
        config: DEFAULT_CONFIG,
        summaryStates: {},
        sourceHistoryId: null,
      });
    });
  });

  it('scrolls to active segment during playback', async () => {
    const segments = [
      { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
      { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
      { id: '3', start: 2, end: 3, text: 'Three', isFinal: true, tokens: [], timestamps: [] },
    ];

    // Set initial state
    act(() => {
      useTranscriptStore.setState({
        segments,
        activeSegmentId: '1',
        activeSegmentIndex: 0,
        isPlaying: false,
      });
    });

    render(<TranscriptEditor />);

    // Start playing
    act(() => {
      useTranscriptStore.setState({ isPlaying: true });
    });

    // Changing active segment should trigger scroll
    act(() => {
      useTranscriptStore.setState({ activeSegmentId: '2', activeSegmentIndex: 1 });
    });

    expect(mockScrollToIndex).toHaveBeenCalledWith(
      expect.objectContaining({
        index: 1,
        align: 'center',
        behavior: 'smooth',
      })
    );
  });

  it('does not scroll if not playing', async () => {
    const segments = [
      { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
      { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
    ];

    act(() => {
      useTranscriptStore.setState({
        segments,
        activeSegmentId: '1',
        isPlaying: false,
      });
    });

    render(<TranscriptEditor />);

    // Change active segment without playing
    act(() => {
      useTranscriptStore.setState({ activeSegmentId: '2' });
    });

    expect(mockScrollToIndex).not.toHaveBeenCalled();
  });

  it('renders only changed segments when context is stable (performance)', async () => {
    const segments = [
      { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
      { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
      { id: '3', start: 2, end: 3, text: 'Three', isFinal: true, tokens: [], timestamps: [] },
    ];

    // Initial render
    act(() => {
      useTranscriptStore.setState({ segments });
    });

    segmentRenderCounts.clear();
    render(<TranscriptEditor />);

    // Initial mount renders each segment exactly once.
    expect(segmentRenderCounts.get('1')).toBe(1);
    expect(segmentRenderCounts.get('2')).toBe(1);
    expect(segmentRenderCounts.get('3')).toBe(1);

    // Update text of first segment
    act(() => {
      useTranscriptStore.getState().updateSegment('1', { text: 'One Updated' });
    });

    // With optimization, context is stable:
    // segment 1 re-renders exactly once, siblings stay memoized.
    expect(segmentRenderCounts.get('1')).toBe(2);
    expect(segmentRenderCounts.get('2')).toBe(1);
    expect(segmentRenderCounts.get('3')).toBe(1);
  });

  it('renders standard empty state when segments are empty regardless of recording', () => {
    act(() => {
      useTranscriptStore.setState({ segments: [], isRecording: true, isPaused: false });
    });

    const { container } = render(<TranscriptEditor />);
    expect(container.querySelector('.empty-state')).toBeTruthy();
    expect(container.querySelector('.transcript-ghost-segment')).toBeNull();
  });

  it('scrolls to end when recording receives new segments and shows resume button when detached', () => {
    const segments = [
      { id: '1', start: 0, end: 1, text: 'Hello', isFinal: true, tokens: [], timestamps: [] },
    ];

    act(() => {
      useTranscriptStore.setState({ segments, isRecording: true, isPaused: false });
    });

    const { container } = render(<TranscriptEditor />);

    // When a second segment arrives during recording, auto-scrolls to bottom
    act(() => {
      useTranscriptStore.setState({
        segments: [
          ...segments,
          { id: '2', start: 1, end: 2, text: 'World', isFinal: false, tokens: [], timestamps: [] },
        ],
      });
    });

    expect(mockScrollToIndex).toHaveBeenCalledWith(
      expect.objectContaining({
        index: 1,
        align: 'end',
      })
    );

    // User scrolls up (virtuoso signals atBottom = false)
    act(() => {
      lastVirtuosoProps.current.atBottomStateChange(false);
    });

    // Resume button appears
    const resumeBtn = container.querySelector('[data-testid="scroll-follow-resume-btn"]');
    expect(resumeBtn).toBeTruthy();

    mockScrollToIndex.mockClear();

    // Clicking resume scrolls to bottom
    act(() => {
      resumeBtn?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(mockScrollToIndex).toHaveBeenCalledWith(
      expect.objectContaining({
        index: 1,
        align: 'end',
        behavior: 'smooth',
      })
    );
  });

  it('triggers search on Ctrl+F / Cmd+F when focused within .panel-right or .projects-detail-pane', () => {
    useSearchStore.setState({ isOpen: false });
    useTranscriptStore.setState({
      segments: [{ id: '1', start: 0, end: 1, text: 'Hello', isFinal: true }],
    });
    const wrapper = document.createElement('div');
    wrapper.className = 'panel-right';
    document.body.appendChild(wrapper);

    const { container } = render(<TranscriptEditor />, { container: wrapper });
    const editor = container.querySelector('.transcript-editor') as HTMLElement;
    expect(editor).toBeTruthy();

    // 1. Focused within .panel-right
    editor.focus();
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    expect(useSearchStore.getState().isOpen).toBe(true);

    useSearchStore.setState({ isOpen: false });

    // 2. Focused on body
    document.body.focus();
    fireEvent.keyDown(window, { key: 'f', metaKey: true });
    expect(useSearchStore.getState().isOpen).toBe(true);

    useSearchStore.setState({ isOpen: false });

    // 3. Focused outside both .panel-right and .projects-detail-pane
    const outsideInput = document.createElement('input');
    document.body.appendChild(outsideInput);
    outsideInput.focus();

    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    expect(useSearchStore.getState().isOpen).toBe(false);

    document.body.removeChild(outsideInput);
    document.body.removeChild(wrapper);
  });

  it('ignores Ctrl+F and does not render SearchUI when isActive is false', () => {
    useSearchStore.setState({ isOpen: false });
    useTranscriptStore.setState({
      segments: [{ id: '1', start: 0, end: 1, text: 'Hello', isFinal: true }],
    });

    const { container } = render(<TranscriptEditor isActive={false} />);
    document.body.focus();
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    expect(useSearchStore.getState().isOpen).toBe(false);

    act(() => {
      useSearchStore.setState({ isOpen: true });
    });
    expect(container.querySelector('.search-ui-container')).toBeNull();
  });
});
