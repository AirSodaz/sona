import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CaptionWindow, computeCaptionTextStyle } from '../CaptionWindow';

const defaultStyle = {
  width: 800,
  fontSize: 22,
  color: '#ffffff',
  backgroundColor: '#17171a',
  backgroundOpacity: 0.72,
};

// Hoist mocks
const mocks = vi.hoisted(() => {
  const listenCallbacks: Record<string, (event: any) => void> = {};
  return {
    listenCallbacks,
    mockListen: vi.fn((event: string, callback: any) => {
      listenCallbacks[event] = callback;
      return Promise.resolve(() => {
        delete listenCallbacks[event];
      });
    }),
    mockSetSize: vi.fn(),
    mockScaleFactor: vi.fn().mockResolvedValue(2.0),
    mockInnerSize: vi.fn().mockResolvedValue({ width: 1600, height: 200 }),
    mockClose: vi.fn(),
    mockStartDragging: vi.fn(),
    mockSetMinSize: vi.fn(),
    mockSetMaxSize: vi.fn(),
    mockSetIgnoreCursorEvents: vi.fn().mockResolvedValue(undefined),
    mockEmit: vi.fn().mockResolvedValue(undefined),
    mockInvoke: vi.fn(async (command: string): Promise<any> => {
      if (command === 'get_aux_window_state') {
        return null;
      }
      return undefined;
    }),
    resizeObserverInstance: null as any,
  };
});

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    setSize: mocks.mockSetSize,
    scaleFactor: mocks.mockScaleFactor,
    innerSize: mocks.mockInnerSize,
    close: mocks.mockClose,
    startDragging: mocks.mockStartDragging,
    setMinSize: mocks.mockSetMinSize,
    setMaxSize: mocks.mockSetMaxSize,
    setIgnoreCursorEvents: mocks.mockSetIgnoreCursorEvents,
  }),
}));

// Mock LogicalSize and PhysicalSize classes (usually imported from @tauri-apps/api/dpi)
vi.mock('@tauri-apps/api/dpi', () => ({
  LogicalSize: class {
    width: number;
    height: number;
    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
    }
  },
  PhysicalSize: class {
    width: number;
    height: number;
    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
    }
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.mockListen,
  emit: mocks.mockEmit,
}));

describe('CaptionWindow', () => {
  beforeEach(() => {
    // Mock ResizeObserver
    globalThis.ResizeObserver = class {
      callback: any;
      target: any;
      constructor(callback: any) {
        this.callback = callback;
        mocks.resizeObserverInstance = this;
      }
      observe(target: any) {
        this.target = target;
        // Trigger callback immediately to simulate initial size observation
        this.callback([{ target, contentRect: { height: target.offsetHeight || 0 } }]);
      }
      unobserve() {}
      disconnect() {}
      triggerResize() {
        if (this.target) {
          this.callback([
            {
              target: this.target,
              contentRect: { height: this.target.getBoundingClientRect().height },
            },
          ]);
        }
      }
    };

    // Clear callbacks
    for (const key in mocks.listenCallbacks) delete mocks.listenCallbacks[key];

    vi.useFakeTimers();
    mocks.mockSetSize.mockClear();
    mocks.mockInvoke.mockImplementation(async (command: string): Promise<any> => {
      if (command === 'get_aux_window_state') {
        return null;
      }
      return undefined;
    });
    mocks.mockEmit.mockClear();
    mocks.mockSetIgnoreCursorEvents.mockClear();
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });

    // Mock offsetHeight and getBoundingClientRect
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 50 });
    HTMLElement.prototype.getBoundingClientRect = () => ({
      width: 800,
      height: 50,
      top: 0,
      left: 0,
      bottom: 50,
      right: 800,
      x: 0,
      y: 0,
      toJSON: () => {},
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('renders empty initially', () => {
    render(<CaptionWindow />);
    expect(screen.queryByText('Waiting for speech...')).toBeNull();
  });

  it('updates to show the last segment', async () => {
    render(<CaptionWindow />);

    // Simulate event
    await act(async () => {
      if (mocks.listenCallbacks['caption:state']) {
        mocks.listenCallbacks['caption:state']({
          payload: {
            revision: 1,
            segments: [
              {
                id: '2',
                text: 'New',
                start: 1,
                end: 2,
                isFinal: false,
                tokens: [],
                timestamps: [],
                durations: [],
              },
            ],
            style: defaultStyle,
          },
        });
      }
    });

    // Should only show "New"
    expect(screen.queryByText('Old')).toBeNull();
    expect(screen.getByText('New')).toBeTruthy();
  });

  it('clears text after disappearDelay and fade transition', async () => {
    const { container } = render(<CaptionWindow />);

    // Add text with disappearDelay: 3.0
    await act(async () => {
      if (mocks.listenCallbacks['caption:state']) {
        mocks.listenCallbacks['caption:state']({
          payload: {
            revision: 1,
            segments: [
              {
                id: '1',
                text: 'Hello',
                start: 0,
                end: 1,
                isFinal: false,
                tokens: [],
                timestamps: [],
                durations: [],
              },
            ],
            style: { ...defaultStyle, disappearDelay: 3.0 },
          },
        });
      }
    });
    expect(screen.getByText('Hello')).toBeTruthy();

    const body = container.querySelector('.caption-window-body');
    expect(body?.classList.contains('fading-out')).toBe(false);

    // Wait 3 seconds -> starts fading out
    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(body?.classList.contains('fading-out')).toBe(true);

    // Wait 300ms fade transition -> cleared
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(screen.queryByText('Waiting for speech...')).toBeNull();
    expect(screen.queryByText('Hello')).toBeNull();
    expect(body?.classList.contains('fading-out')).toBe(false);
  });

  it('resizes window when content changes', async () => {
    render(<CaptionWindow />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(mocks.mockSetSize).toHaveBeenCalled();
    const initialCall = mocks.mockSetSize.mock.calls[mocks.mockSetSize.mock.calls.length - 1]?.[0];
    expect(initialCall?.width).toBe(1600);
    expect(initialCall?.height).toBe(100);

    mocks.mockSetSize.mockClear();
    HTMLElement.prototype.getBoundingClientRect = () => ({
      width: 800,
      height: 150,
      top: 0,
      left: 0,
      bottom: 150,
      right: 800,
      x: 0,
      y: 0,
      toJSON: () => {},
    });

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: '1',
              text: 'Hello',
              start: 0,
              end: 1,
              isFinal: false,
              tokens: [],
              timestamps: [],
              durations: [],
            },
          ],
          style: defaultStyle,
        },
      });
      mocks.resizeObserverInstance.triggerResize();
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(mocks.mockSetSize).toHaveBeenCalled();
    const resizedCall = mocks.mockSetSize.mock.calls[mocks.mockSetSize.mock.calls.length - 1]?.[0];
    expect(resizedCall?.height).toBe(300);

    mocks.mockSetSize.mockClear();
    HTMLElement.prototype.getBoundingClientRect = () => ({
      width: 960,
      height: 90,
      top: 0,
      left: 0,
      bottom: 90,
      right: 960,
      x: 0,
      y: 0,
      toJSON: () => {},
    });

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 2,
          segments: [
            {
              id: '2',
              text: 'Wider caption',
              start: 1,
              end: 2,
              isFinal: false,
              tokens: [],
              timestamps: [],
              durations: [],
            },
          ],
          style: {
            ...defaultStyle,
            width: 960,
          },
        },
      });
    });

    mocks.mockSetSize.mockClear();

    await act(async () => {
      mocks.resizeObserverInstance.triggerResize();
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(mocks.mockSetSize).toHaveBeenCalled();
    const widthChangedCall =
      mocks.mockSetSize.mock.calls[mocks.mockSetSize.mock.calls.length - 1]?.[0];
    expect(widthChangedCall?.width).toBe(1920);
    expect(widthChangedCall?.height).toBe(180);
  });

  it('applies custom background color and opacity as rgba', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 2,
          segments: [],
          style: {
            ...defaultStyle,
            backgroundColor: '#ff0000',
            backgroundOpacity: 0.8,
          },
        },
      });
    });

    const body = container.querySelector('.caption-window-body') as HTMLElement;
    expect(body.style.background).toBe('rgba(255, 0, 0, 0.8)');
  });

  it('renders bilingual subtitle with translation prefix and draft tail', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-1',
              text: 'Hello world',
              translation: '你好',
              draftTranslation: '世界',
              start: 0,
              end: 1,
              isFinal: false,
            },
          ],
          style: {
            ...defaultStyle,
            displayMode: 'bilingual',
            bilingualOrder: 'original_first',
          },
        },
      });
    });

    expect(screen.getByText('Hello world')).toBeDefined();
    expect(screen.getByText('你好')).toBeDefined();
    expect(screen.getByText('世界')).toBeDefined();
    const draftTail = container.querySelector('.caption-draft-tail');
    expect(draftTail).not.toBeNull();
    expect(draftTail?.textContent).toBe('世界');
  });

  it('respects displayMode translation only and original only', async () => {
    render(<CaptionWindow />);

    // Translation only
    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-1',
              text: 'Good morning',
              translation: '早上好',
              start: 0,
              end: 1,
              isFinal: true,
            },
          ],
          style: {
            ...defaultStyle,
            displayMode: 'translation',
          },
        },
      });
    });

    expect(screen.getByText('早上好')).toBeDefined();
    expect(screen.queryByText('Good morning')).toBeNull();

    // Original only
    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 2,
          segments: [
            {
              id: 'seg-2',
              text: 'Good morning',
              translation: '早上好',
              start: 0,
              end: 1,
              isFinal: true,
            },
          ],
          style: {
            ...defaultStyle,
            displayMode: 'original',
          },
        },
      });
    });

    expect(screen.getByText('Good morning')).toBeDefined();
    expect(screen.queryByText('早上好')).toBeNull();
  });

  it('renders pending fallback placeholder with original text when translation is missing in translation mode', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-pending',
              text: 'Pending translation text',
              translation: null,
              draftTranslation: null,
              start: 0,
              end: 1,
              isFinal: false,
            },
          ],
          style: {
            ...defaultStyle,
            displayMode: 'translation',
          },
        },
      });
    });

    const pendingEl = container.querySelector('.caption-translation-pending');
    expect(pendingEl).not.toBeNull();
    expect(pendingEl?.textContent).toContain('Pending translation text');
    expect((pendingEl as HTMLElement).style.opacity).toBe('0.6');
  });

  it('displays error banner when realtime-translation-error event is received', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['realtime-translation-error']?.({
        payload: {
          error: 'Model opus-mt-en-zh not installed',
          engine: 'onnx',
        },
      });
    });

    const errorBanner = container.querySelector('.caption-error-banner');
    expect(errorBanner).not.toBeNull();
    expect(errorBanner?.textContent).toContain('Model opus-mt-en-zh not installed');
  });

  it('applies text stroke and diffuse shadow when textStroke is true', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-stroke',
              text: 'Stroke test',
              start: 0,
              end: 1,
              isFinal: true,
            },
          ],
          style: {
            ...defaultStyle,
            textStroke: true,
            strokeWidth: 3,
            strokeColor: '#000000',
            textShadow: true,
          },
        },
      });
    });

    const originalLine = container.querySelector('.caption-original-line') as HTMLElement;
    expect(originalLine).not.toBeNull();
    expect(originalLine.style.webkitTextStroke).toBe('3px #000000');
    expect(originalLine.style.textShadow).toContain('-1px -1px 0 #000000');
    expect(originalLine.style.textShadow).toContain('0 1px 3px rgba(0, 0, 0, 0.55)');
  });

  it('renders translation_main with translation first, larger font, and original below', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-prio',
              text: 'Original English',
              translation: '中文译文',
              start: 0,
              end: 1,
              isFinal: true,
            },
          ],
          style: {
            ...defaultStyle,
            displayPriority: 'translation_main',
            fontSize: 28,
            translationFontSize: 18,
            translationColor: '#10b981',
          },
        },
      });
    });

    const translationLine = container.querySelector('.caption-translation-line') as HTMLElement;
    const originalLine = container.querySelector('.caption-original-line') as HTMLElement;
    expect(translationLine).not.toBeNull();
    expect(originalLine).not.toBeNull();

    // Translation is primary: 28px and translationColor
    expect(translationLine.style.fontSize).toBe('28px');
    expect(translationLine.style.color).toBe('rgb(16, 185, 129)');

    // Original is secondary: 18px and 0.8 opacity
    expect(originalLine.style.fontSize).toBe('18px');
    expect(originalLine.style.opacity).toBe('0.8');

    // DOM order: translation line appears before original line
    expect(
      translationLine.compareDocumentPosition(originalLine) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
  });

  it('does not render original line in translation_only display priority', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-trans-only',
              text: 'Original hidden text',
              translation: 'Visible translation text',
              start: 0,
              end: 1,
              isFinal: true,
            },
          ],
          style: {
            ...defaultStyle,
            displayPriority: 'translation_only',
          },
        },
      });
    });

    expect(screen.getByText('Visible translation text')).toBeDefined();
    expect(screen.queryByText('Original hidden text')).toBeNull();
    expect(container.querySelector('.caption-original-line')).toBeNull();
  });

  it('immediately interrupts fading-out state when new segment arrives', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-1',
              text: 'Sentence one',
              start: 0,
              end: 1,
              isFinal: true,
            },
          ],
          style: {
            ...defaultStyle,
            disappearDelay: 4.0,
          },
        },
      });
    });

    const body = container.querySelector('.caption-window-body') as HTMLElement;
    expect(body.classList.contains('fading-out')).toBe(false);

    // Advance 4s to trigger fade-out
    await act(async () => {
      vi.advanceTimersByTime(4000);
    });
    expect(body.classList.contains('fading-out')).toBe(true);

    // New segment arrives mid-fade!
    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 2,
          segments: [
            {
              id: 'seg-2',
              text: 'Sentence two',
              start: 1,
              end: 2,
              isFinal: false,
            },
          ],
          style: {
            ...defaultStyle,
            disappearDelay: 4.0,
          },
        },
      });
    });

    // fading-out is immediately cleared, sentence two is visible
    expect(body.classList.contains('fading-out')).toBe(false);
    expect(screen.getByText('Sentence two')).toBeDefined();
  });

  it('copies context-aware text to clipboard and displays toast', async () => {
    render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-1',
              text: 'Hello world',
              translation: '你好世界',
              start: 0,
              end: 1,
              isFinal: true,
            },
          ],
          style: {
            ...defaultStyle,
            displayPriority: 'translation_main',
          },
        },
      });
    });

    const copyBtn = screen.getByTitle('Copy Subtitle');
    await act(async () => {
      fireEvent.click(copyBtn);
    });

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('你好世界');
    expect(screen.getByText('Copied')).toBeDefined();

    // Advance timer to verify toast disappears
    act(() => {
      vi.advanceTimersByTime(2100);
    });
    expect(screen.queryByText('Copied')).toBeNull();
  });

  it('locks window, emits caption:lock-toggle, and shows toast banner', async () => {
    render(<CaptionWindow />);

    const lockBtn = screen.getByTitle('Lock click-through (Alt+C)');
    await act(async () => {
      fireEvent.click(lockBtn);
    });

    expect(screen.getByText('Click-through enabled (Alt+C to unlock)')).toBeDefined();
    expect(mocks.mockEmit).not.toHaveBeenCalled();

    // Advance timers past 1500ms lock delay
    await act(async () => {
      vi.advanceTimersByTime(1600);
    });

    expect(mocks.mockEmit).toHaveBeenCalledWith('caption:lock-toggle', true);
    expect(mocks.mockSetIgnoreCursorEvents).toHaveBeenCalledWith(true);
  });

  it('cleans up active toast and lock timers on unmount', async () => {
    const { unmount } = render(<CaptionWindow />);

    const lockBtn = screen.getByTitle('Lock click-through (Alt+C)');
    await act(async () => {
      fireEvent.click(lockBtn);
    });

    unmount();

    // Advancing timers after unmount does not trigger further effects or throw
    await act(async () => {
      vi.advanceTimersByTime(2500);
    });
  });

  it('does not apply inline backdropFilter to caption-window-body to prevent corner artifacts', async () => {
    const { container } = render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [],
          style: defaultStyle,
        },
      });
    });

    const body = container.querySelector('.caption-window-body') as HTMLElement;
    expect(body.style.backdropFilter).toBeFalsy();
  });

  it('copies joined translation and draftTranslation to clipboard', async () => {
    render(<CaptionWindow />);

    await act(async () => {
      mocks.listenCallbacks['caption:state']?.({
        payload: {
          revision: 1,
          segments: [
            {
              id: 'seg-draft',
              text: 'Hello world',
              translation: 'Hello',
              draftTranslation: 'world',
              start: 0,
              end: 1,
              isFinal: false,
            },
          ],
          style: {
            ...defaultStyle,
            displayPriority: 'translation_main',
          },
        },
      });
    });

    const copyBtn = screen.getByTitle('Copy Subtitle');
    await act(async () => {
      fireEvent.click(copyBtn);
    });

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('Hello world');
    expect(screen.getByText('Copied')).toBeDefined();
  });
});
describe('computeCaptionTextStyle', () => {
  it('omits stroke and hard shadow when stroke is disabled or width <= 0', () => {
    const style = {
      ...defaultStyle,
      textStroke: false,
      strokeWidth: 0,
      textShadow: true,
    };
    const primary = computeCaptionTextStyle(style, true, false);
    expect(primary.WebkitTextStroke).toBeUndefined();
    expect(primary.textShadow).toBe('0 1px 3px rgba(0, 0, 0, 0.55)');
    expect(primary.textShadow).not.toContain('-1px');

    const translation = computeCaptionTextStyle(style, false, true);
    expect(translation.WebkitTextStroke).toBeUndefined();
    expect(translation.textShadow).toBe('0 1px 2px rgba(0, 0, 0, 0.4)');
  });

  it('applies 4-cardinal offsets and diffuse shadow when stroke is enabled with width > 0', () => {
    const style = {
      ...defaultStyle,
      textStroke: true,
      strokeWidth: 2,
      strokeColor: '#ff0000',
      textShadow: true,
    };
    const primary = computeCaptionTextStyle(style, true, false);
    expect(primary.WebkitTextStroke).toBe('2px #ff0000');
    expect(primary.textShadow).toContain('-1px -1px 0 #ff0000');
    expect(primary.textShadow).toContain('1px 1px 0 #ff0000');
    expect(primary.textShadow).toContain('0 1px 3px rgba(0, 0, 0, 0.55)');
    expect(primary.textShadow).not.toContain('-2px 0 0');
  });

  it('omits text shadow completely when textShadow is false', () => {
    const style = {
      ...defaultStyle,
      textStroke: false,
      strokeWidth: 0,
      textShadow: false,
    };
    const primary = computeCaptionTextStyle(style, true, false);
    expect(primary.textShadow).toBeUndefined();
  });
});
