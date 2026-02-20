import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { CaptionWindow } from '../CaptionWindow';

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
    };
});

vi.mock('@tauri-apps/api/window', () => ({
    getCurrentWindow: () => ({
        setSize: mocks.mockSetSize,
        scaleFactor: mocks.mockScaleFactor,
        innerSize: mocks.mockInnerSize,
        close: mocks.mockClose,
        startDragging: mocks.mockStartDragging,
    }),
}));

// Mock LogicalSize class (usually imported from @tauri-apps/api/dpi)
vi.mock('@tauri-apps/api/dpi', () => ({
    LogicalSize: class {
        width: number;
        height: number;
        constructor(width: number, height: number) {
            this.width = width;
            this.height = height;
        }
    }
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.mockListen
}));

describe('CaptionWindow', () => {
    beforeEach(() => {
        // Clear callbacks
        for (const key in mocks.listenCallbacks) delete mocks.listenCallbacks[key];

        vi.useFakeTimers();
        mocks.mockSetSize.mockClear();

        // Mock scrollHeight
        Object.defineProperty(HTMLElement.prototype, 'scrollHeight', { configurable: true, value: 50 });
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
    });

    it('renders waiting message initially', () => {
        render(<CaptionWindow />);
        expect(screen.getByText('Waiting for speech...')).toBeTruthy();
    });

    it('updates to show the last segment', async () => {
        render(<CaptionWindow />);

        // Simulate event
        await act(async () => {
            if (mocks.listenCallbacks['caption:segments']) {
                mocks.listenCallbacks['caption:segments']({
                    payload: [
                        { id: '1', text: 'Old', start: 0, end: 1, isFinal: true, tokens: [], timestamps: [], durations: [] },
                        { id: '2', text: 'New', start: 1, end: 2, isFinal: false, tokens: [], timestamps: [], durations: [] }
                    ]
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
            if (mocks.listenCallbacks['caption:segments']) {
                mocks.listenCallbacks['caption:segments']({
                    payload: [{ id: '1', text: 'Hello', start: 0, end: 1, isFinal: false, tokens: [], timestamps: [], durations: [] }]
                });
            }
        });
        expect(screen.getByText('Hello')).toBeTruthy();

        // Wait 3 seconds
        await act(async () => {
            vi.advanceTimersByTime(3000);
        });

        // Should be cleared (back to waiting or empty?)
        // The code renders "Waiting for speech..." if segments is empty.
        expect(screen.getByText('Waiting for speech...')).toBeTruthy();
        expect(screen.queryByText('Hello')).toBeNull();
    });

    it('resizes window when content changes', async () => {
        render(<CaptionWindow />);

        // Initial render triggers layout effect with "Waiting for speech..."
        // scrollHeight mocked to 50. Total height = 50 + 32 = 82.
        // innerSize width 1600 / scale 2 = 800.
        // So setSize(800, 82).

        // We need to wait for async calls in useLayoutEffect
        await act(async () => {
            await Promise.resolve(); // flush microtasks
            await Promise.resolve();
        });

        expect(mocks.mockSetSize).toHaveBeenCalled();
        const call = mocks.mockSetSize.mock.calls[0][0];
        expect(call.width).toBe(800);
        expect(call.height).toBe(82);

        // Update content
        mocks.mockSetSize.mockClear();
        // Change scrollHeight for next render
        // Note: Defining property on prototype changes it for all elements.
        Object.defineProperty(HTMLElement.prototype, 'scrollHeight', { configurable: true, value: 100 });

        await act(async () => {
            if (mocks.listenCallbacks['caption:segments']) {
                mocks.listenCallbacks['caption:segments']({
                    payload: [{ id: '1', text: 'Hello', start: 0, end: 1, isFinal: false, tokens: [], timestamps: [], durations: [] }]
                });
            }
        });

        // Wait for async effect
        await act(async () => {
            await Promise.resolve();
            await Promise.resolve();
        });

        // Should resize again
        // 100 + 32 = 132
        expect(mocks.mockSetSize).toHaveBeenCalled();
        const call2 = mocks.mockSetSize.mock.calls[0][0];
        expect(call2.height).toBe(132);
    });
});
