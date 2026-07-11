import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
    const emitTo = vi.fn().mockResolvedValue(undefined);
    const invoke = vi.fn().mockResolvedValue(undefined);
    const getByLabel = vi.fn().mockResolvedValue(null);
    const createdWindows: any[] = [];
    const nextWindowFailures = {
        setAlwaysOnTop: null as unknown,
        setIgnoreCursorEvents: null as unknown,
    };

    class MockPhysicalSize {
        constructor(
            public width: number,
            public height: number
        ) { }
    }

    const buildWindow = () => {
        const windowInstance = {
        once: vi.fn((event: string, callback: (payload?: unknown) => void) => {
            if (event === 'tauri://created') {
                callback();
            }
        }),
        close: vi.fn().mockResolvedValue(undefined),
        hide: vi.fn().mockResolvedValue(undefined),
        show: vi.fn().mockResolvedValue(undefined),
        setFocus: vi.fn().mockResolvedValue(undefined),
        setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
        setIgnoreCursorEvents: vi.fn().mockResolvedValue(undefined),
        scaleFactor: vi.fn().mockResolvedValue(1),
        innerSize: vi.fn().mockResolvedValue({ width: 800, height: 120 }),
        setSize: vi.fn().mockResolvedValue(undefined),
        };

        if (nextWindowFailures.setAlwaysOnTop) {
            windowInstance.setAlwaysOnTop.mockRejectedValueOnce(nextWindowFailures.setAlwaysOnTop);
        }
        if (nextWindowFailures.setIgnoreCursorEvents) {
            windowInstance.setIgnoreCursorEvents.mockRejectedValueOnce(nextWindowFailures.setIgnoreCursorEvents);
        }

        return windowInstance;
    };

    class MockWebviewWindow {
        label: string;
        options: Record<string, unknown>;
        windowInstance: ReturnType<typeof buildWindow>;

        constructor(label: string, options: Record<string, unknown>) {
            this.label = label;
            this.options = options;
            this.windowInstance = buildWindow();
            createdWindows.push(this);
            return this.windowInstance as any;
        }

        static getByLabel(label: string) {
            return getByLabel(label);
        }
    }

    return {
        MockPhysicalSize,
        MockWebviewWindow,
        buildWindow,
        createdWindows,
        emitTo,
        getByLabel,
        invoke,
        nextWindowFailures,
    };
});

vi.mock('@tauri-apps/api/core', () => ({
    invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
    emitTo: mocks.emitTo,
}));

vi.mock('@tauri-apps/api/dpi', () => ({
    PhysicalSize: mocks.MockPhysicalSize,
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
    WebviewWindow: mocks.MockWebviewWindow,
}));

vi.mock('../../utils/logger', () => ({
    logger: {
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
        debug: vi.fn(),
    },
}));

describe('CaptionWindowService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.resetModules();
        mocks.createdWindows.length = 0;
        mocks.getByLabel.mockResolvedValue(null);
        mocks.invoke.mockResolvedValue(undefined);
        mocks.emitTo.mockResolvedValue(undefined);
        mocks.nextWindowFailures.setAlwaysOnTop = null;
        mocks.nextWindowFailures.setIgnoreCursorEvents = null;
    });

    it('opens the window with the default caption state and shared transport payload', async () => {
        const { captionWindowService } = await import('../captionWindowService');

        await captionWindowService.open();

        expect(mocks.createdWindows).toHaveLength(1);
        expect(mocks.createdWindows[0].options).toEqual(
            expect.objectContaining({
                url: '/index.html?window=caption',
                width: 800,
                height: 120,
                resizable: true,
                maximizable: false,
                minimizable: false,
                transparent: true,
                decorations: false,
                alwaysOnTop: true,
                visible: true,
            })
        );
        expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
            label: 'caption',
            payload: {
                revision: 1,
                segments: [],
                style: {
                    width: 800,
                    fontSize: 24,
                    color: '#ffffff',
                    backgroundOpacity: 0.6,
                },
            },
        });
        expect(mocks.emitTo).toHaveBeenCalledWith({
            kind: 'AnyLabel',
            label: 'caption',
        }, 'caption:state', {
            revision: 1,
            segments: [],
            style: {
                width: 800,
                fontSize: 24,
                color: '#ffffff',
                backgroundOpacity: 0.6,
            },
        });
    });

    it('opens the window with custom style properties and applies window flags', async () => {
        const { captionWindowService } = await import('../captionWindowService');

        await captionWindowService.open({
            width: 1000,
            fontSize: 32,
            color: '#ff0000',
            backgroundOpacity: 0.75,
            alwaysOnTop: false,
            lockWindow: true,
        });

        const createdWindow = mocks.createdWindows[0].windowInstance;

        expect(mocks.createdWindows[0].options).toEqual(
            expect.objectContaining({
                url: '/index.html?window=caption',
                width: 1000,
                alwaysOnTop: false,
            })
        );
        expect(createdWindow.setAlwaysOnTop).not.toHaveBeenCalled();
        expect(createdWindow.setIgnoreCursorEvents).toHaveBeenCalledWith(true);
        expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
            label: 'caption',
            payload: {
                revision: 1,
                segments: [],
                style: {
                    width: 1000,
                    fontSize: 32,
                    color: '#ff0000',
                    backgroundOpacity: 0.75,
                },
            },
        });
    });

    it('keeps opening when post-create caption window flags hit a stale window handle', async () => {
        mocks.nextWindowFailures.setIgnoreCursorEvents = new Error('window not found');

        const { captionWindowService } = await import('../captionWindowService');

        await expect(captionWindowService.open({
            lockWindow: true,
        })).resolves.toBeUndefined();

        expect(mocks.createdWindows).toHaveLength(1);
        expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
            label: 'caption',
            payload: {
                revision: 1,
                segments: [],
                style: {
                    width: 800,
                    fontSize: 24,
                    color: '#ffffff',
                    backgroundOpacity: 0.6,
                },
            },
        });
        expect(mocks.createdWindows[0].windowInstance.setIgnoreCursorEvents).toHaveBeenCalledWith(true);
    });

    it('ignores stale-window failures when applying caption window flags later', async () => {
        const existingWindow = mocks.buildWindow();
        existingWindow.setAlwaysOnTop.mockRejectedValueOnce(new Error('window not found'));
        existingWindow.setIgnoreCursorEvents.mockRejectedValueOnce(new Error('window not found'));
        mocks.getByLabel.mockResolvedValue(existingWindow);

        const { captionWindowService } = await import('../captionWindowService');

        await expect(captionWindowService.setAlwaysOnTop(false)).resolves.toBeUndefined();
        await expect(captionWindowService.setClickThrough(true)).resolves.toBeUndefined();

        expect(existingWindow.setAlwaysOnTop).toHaveBeenCalledWith(false);
        expect(existingWindow.setIgnoreCursorEvents).toHaveBeenCalledWith(true);
    });

    it('reuses an existing window and keeps style updates flowing through shared state without resizing from the service', async () => {
        const existingWindow = mocks.buildWindow();
        mocks.getByLabel.mockResolvedValue(existingWindow);

        const { captionWindowService } = await import('../captionWindowService');

        await captionWindowService.open({ width: 1200, fontSize: 40 });

        expect(mocks.createdWindows).toHaveLength(0);
        expect(existingWindow.show).toHaveBeenCalled();
        expect(existingWindow.setFocus).toHaveBeenCalled();
        expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
            label: 'caption',
            payload: {
                revision: 1,
                segments: [],
                style: {
                    width: 1200,
                    fontSize: 40,
                    color: '#ffffff',
                    backgroundOpacity: 0.6,
                },
            },
        });

        vi.clearAllMocks();
        mocks.getByLabel.mockResolvedValue(existingWindow);

        await captionWindowService.updateStyle({ width: 1400, fontSize: 42 });

        expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
            label: 'caption',
            payload: {
                revision: 2,
                segments: [],
                style: {
                    width: 1400,
                    fontSize: 42,
                    color: '#ffffff',
                    backgroundOpacity: 0.6,
                },
            },
        });
        expect(existingWindow.setSize).not.toHaveBeenCalled();
    });

    it('skips committing style updates when the caption style is unchanged', async () => {
        const { captionWindowService } = await import('../captionWindowService');

        await captionWindowService.updateStyle({
            width: 800,
            fontSize: 24,
            color: '#ffffff',
            backgroundOpacity: 0.6,
        });

        expect(mocks.invoke).not.toHaveBeenCalled();
        expect(mocks.emitTo).not.toHaveBeenCalled();
        expect(mocks.getByLabel).not.toHaveBeenCalled();
    });
});
