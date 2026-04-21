import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
    const emitTo = vi.fn().mockResolvedValue(undefined);
    const invoke = vi.fn().mockResolvedValue(undefined);
    const getByLabel = vi.fn().mockResolvedValue(null);
    const createdWindows: any[] = [];

    class MockPhysicalPosition {
        constructor(
            public x: number,
            public y: number
        ) { }
    }

    class MockPhysicalSize {
        constructor(
            public width: number,
            public height: number
        ) { }
    }

    class MockWebviewWindow {
        label: string;
        options: Record<string, unknown>;
        onceHandlers: Record<string, (payload?: unknown) => void> = {};
        setSize = vi.fn().mockResolvedValue(undefined);
        setPosition = vi.fn().mockResolvedValue(undefined);
        show = vi.fn().mockResolvedValue(undefined);
        hide = vi.fn().mockResolvedValue(undefined);
        close = vi.fn().mockResolvedValue(undefined);

        constructor(label: string, options: Record<string, unknown>) {
            this.label = label;
            this.options = options;
            createdWindows.push(this);
        }

        once(event: string, callback: (payload?: unknown) => void) {
            this.onceHandlers[event] = callback;
            if (event === 'tauri://created') {
                callback();
            }
        }

        static getByLabel(label: string) {
            return getByLabel(label);
        }
    }

    return {
        MockPhysicalPosition,
        MockPhysicalSize,
        MockWebviewWindow,
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

vi.mock('@tauri-apps/api/webviewWindow', () => ({
    WebviewWindow: mocks.MockWebviewWindow,
}));

vi.mock('@tauri-apps/api/dpi', () => ({
    PhysicalPosition: mocks.MockPhysicalPosition,
    PhysicalSize: mocks.MockPhysicalSize,
}));

vi.mock('../../utils/logger', () => ({
    logger: {
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
        debug: vi.fn(),
    },
}));

describe('voiceTypingWindowService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.resetModules();
        mocks.createdWindows.length = 0;
        mocks.getByLabel.mockResolvedValue(null);
        mocks.invoke.mockResolvedValue(undefined);
        mocks.emitTo.mockResolvedValue(undefined);
    });

    it('prepares a hidden overlay window ahead of time with the standard size', async () => {
        const { voiceTypingWindowService } = await import('../voiceTypingWindowService');

        await voiceTypingWindowService.prepare([120, 220]);

        expect(mocks.createdWindows).toHaveLength(1);
        expect(mocks.createdWindows[0].options).toEqual(
            expect.objectContaining({
                url: '/index.html?window=voice-typing',
                visible: false,
                width: 400,
                height: 60,
            })
        );
        expect(mocks.createdWindows[0].setSize).toHaveBeenCalledWith(
            expect.objectContaining({ width: 400, height: 60 })
        );
        expect(mocks.createdWindows[0].setPosition).toHaveBeenCalledWith(
            expect.objectContaining({ x: 120, y: 220 })
        );
        expect(mocks.createdWindows[0].hide).toHaveBeenCalled();
    });

    it('commits the latest overlay payload to the shared state store and emits it to the overlay window', async () => {
        const { voiceTypingWindowService, VOICE_TYPING_EVENT_TEXT } = await import(
            '../voiceTypingWindowService'
        );

        const payload = {
            sessionId: 'voice-typing-1',
            phase: 'segment' as const,
            text: '整句草稿',
            segmentId: 'seg-1',
            isFinal: false,
            revision: 3,
        };

        await voiceTypingWindowService.sendState(payload);

        expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
            label: 'voice-typing',
            payload,
        });
        expect(mocks.emitTo).toHaveBeenCalledWith('voice-typing', VOICE_TYPING_EVENT_TEXT, payload);
    });

    it('reuses the prepared window when opening the overlay', async () => {
        const { voiceTypingWindowService } = await import('../voiceTypingWindowService');

        await voiceTypingWindowService.prepare([10, 20]);
        const preparedWindow = mocks.createdWindows[0];

        await voiceTypingWindowService.open(80, 160);

        expect(mocks.createdWindows).toHaveLength(1);
        expect(preparedWindow.setPosition).toHaveBeenLastCalledWith(
            expect.objectContaining({ x: 80, y: 160 })
        );
        expect(preparedWindow.setSize).toHaveBeenLastCalledWith(
            expect.objectContaining({ width: 400, height: 60 })
        );
        expect(preparedWindow.show).toHaveBeenCalled();
    });
});
