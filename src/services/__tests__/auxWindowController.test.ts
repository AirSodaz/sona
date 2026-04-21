import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AuxWindowController } from '../auxWindowController';

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
        setFocus = vi.fn().mockResolvedValue(undefined);

        constructor(label: string, options: Record<string, unknown>) {
            this.label = label;
            this.options = options;
            createdWindows.push(this);
        }

        once(event: string, callback: (payload?: unknown) => void) {
            this.onceHandlers[event] = callback;
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

describe('AuxWindowController', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.resetModules();
        mocks.createdWindows.length = 0;
        mocks.getByLabel.mockResolvedValue(null);
    });

    it('returns a newly created visible window even if tauri://created has not fired yet', async () => {
        const controller = new AuxWindowController<{ revision: number }>({
            label: 'caption',
            eventName: 'caption:state',
            createWindow: (displayState, creationState) =>
                new mocks.MockWebviewWindow('caption', {
                    width: displayState.size?.width ?? 0,
                    height: displayState.size?.height ?? 0,
                    visible: creationState.visible,
                }) as any,
        });

        const windowInstance = await controller.open({
            size: { width: 800, height: 120 },
            focus: true,
        });

        expect(windowInstance).toBeTruthy();
        expect(mocks.createdWindows).toHaveLength(1);
        expect(mocks.createdWindows[0].options).toEqual(
            expect.objectContaining({
                width: 800,
                height: 120,
                visible: true,
            })
        );
        expect(mocks.createdWindows[0].show).not.toHaveBeenCalled();
    });

    it('emits to the concrete webview window target when one already exists', async () => {
        const existingWindow = new mocks.MockWebviewWindow('caption', {});
        mocks.getByLabel.mockResolvedValue(existingWindow);

        const controller = new AuxWindowController<{ revision: number }>({
            label: 'caption',
            eventName: 'caption:state',
            createWindow: () => new mocks.MockWebviewWindow('caption', {}) as any,
        });

        await controller.commitState({ revision: 2 });

        expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
            label: 'caption',
            payload: { revision: 2 },
        });
        expect(mocks.emitTo).toHaveBeenCalledWith(
            { kind: 'WebviewWindow', label: 'caption' },
            'caption:state',
            { revision: 2 }
        );
    });
});
