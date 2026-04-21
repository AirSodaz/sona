import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
    const emit = vi.fn().mockResolvedValue(undefined);
    const getByLabel = vi.fn().mockResolvedValue(null);
    const createdWindows: any[] = [];

    class MockWebviewWindow {
        label: string;
        options: Record<string, unknown>;
        onceHandlers: Record<string, (payload?: unknown) => void> = {};
        setPosition = vi.fn().mockResolvedValue(undefined);
        show = vi.fn().mockResolvedValue(undefined);
        hide = vi.fn().mockResolvedValue(undefined);

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
        MockWebviewWindow,
        createdWindows,
        emit,
        getByLabel,
    };
});

vi.mock('@tauri-apps/api/event', () => ({
    emit: mocks.emit,
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
    WebviewWindow: mocks.MockWebviewWindow,
}));

vi.mock('@tauri-apps/api/dpi', () => ({
    PhysicalPosition: class PhysicalPosition {
        constructor(public x: number, public y: number) { }
    },
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
    });

    it('prepares a hidden overlay window ahead of time', async () => {
        const serviceModule = await import('../voiceTypingWindowService');
        const { voiceTypingWindowService } = serviceModule;

        await voiceTypingWindowService.prepare([120, 220]);

        expect(mocks.createdWindows).toHaveLength(1);
        expect(mocks.createdWindows[0].options.visible).toBe(false);
    });

    it('broadcasts overlay updates without waiting for a ready handshake', async () => {
        const serviceModule = await import('../voiceTypingWindowService');
        const { voiceTypingWindowService, VOICE_TYPING_EVENT_TEXT } = serviceModule;

        await voiceTypingWindowService.sendState({
            sessionId: 'voice-typing-1',
            phase: 'segment',
            text: '整句草稿',
            segmentId: 'seg-1',
            isFinal: false,
        });

        expect(mocks.emit).toHaveBeenCalledWith(VOICE_TYPING_EVENT_TEXT, {
            sessionId: 'voice-typing-1',
            phase: 'segment',
            text: '整句草稿',
            segmentId: 'seg-1',
            isFinal: false,
        });
    });

    it('reuses the prepared window when opening the overlay', async () => {
        const serviceModule = await import('../voiceTypingWindowService');
        const { voiceTypingWindowService } = serviceModule;

        await voiceTypingWindowService.prepare([10, 20]);
        const preparedWindow = mocks.createdWindows[0];

        await voiceTypingWindowService.open(80, 160);

        expect(preparedWindow.setPosition).toHaveBeenCalled();
        expect(preparedWindow.show).toHaveBeenCalled();
        expect(mocks.createdWindows).toHaveLength(1);
    });
});
