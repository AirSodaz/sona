import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { CaptionWindow } from '../CaptionWindow';

const defaultStyle = {
    width: 800,
    fontSize: 24,
    color: '#ffffff',
    backgroundOpacity: 0.6,
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
    }
}));

vi.mock('@tauri-apps/api/core', () => ({
    invoke: mocks.mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.mockListen
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
            unobserve() { }
            disconnect() { }
            triggerResize() {
                 if (this.target) {
                     this.callback([{ target: this.target, contentRect: { height: this.target.getBoundingClientRect().height } }]);
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

        // Mock offsetHeight and getBoundingClientRect
        Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 50 });
        HTMLElement.prototype.getBoundingClientRect = () => ({
            width: 800, height: 50, top: 0, left: 0, bottom: 50, right: 800, x: 0, y: 0, toJSON: () => { }
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
                    }
                });
            }
        });

        // Should only show "New"
        expect(screen.queryByText('Old')).toBeNull();
        expect(screen.getByText('New')).toBeTruthy();
    });

    it('clears text after 3 seconds of inactivity', async () => {
        render(<CaptionWindow />);

        // Add text
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
                        style: defaultStyle,
                    }
                });
            }
        });
        expect(screen.getByText('Hello')).toBeTruthy();

        // Wait 3 seconds
        await act(async () => {
            vi.advanceTimersByTime(3000);
        });

        // Should be cleared
        expect(screen.queryByText('Waiting for speech...')).toBeNull();
        expect(screen.queryByText('Hello')).toBeNull();
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
            width: 800, height: 150, top: 0, left: 0, bottom: 150, right: 800, x: 0, y: 0, toJSON: () => { }
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
                }
            });
            mocks.resizeObserverInstance.triggerResize();
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mocks.mockSetSize).toHaveBeenCalled();
        const resizedCall = mocks.mockSetSize.mock.calls[mocks.mockSetSize.mock.calls.length - 1]?.[0];
        expect(resizedCall?.height).toBe(300);
    });
});
