import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
    const emitTo = vi.fn().mockResolvedValue(undefined);
    const invoke = vi.fn().mockResolvedValue(undefined);
    const getByLabel = vi.fn().mockResolvedValue(null);
    const createdWindows: any[] = [];

    class MockPhysicalSize {
        constructor(
            public width: number,
            public height: number
        ) { }
    }

    const buildWindow = () => ({
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
    });

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
                visible: false,
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
        expect(mocks.emitTo).toHaveBeenCalledWith('caption', 'caption:state', {
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
            })
        );
        expect(createdWindow.setAlwaysOnTop).toHaveBeenCalledWith(false);
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

    it('reuses an existing window and keeps style updates flowing through shared state', async () => {
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
        expect(existingWindow.setSize).toHaveBeenCalledWith(
            expect.objectContaining({ width: 1400, height: 120 })
        );
    });
});
