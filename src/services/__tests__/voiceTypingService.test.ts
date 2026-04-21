import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
    defaultConfig: {
        voiceTypingEnabled: false,
        voiceTypingShortcut: 'Alt+V',
        voiceTypingMode: 'hold',
        streamingModelPath: 'path/to/model',
        language: 'auto',
        enableITN: true,
        microphoneId: 'default',
    },
    config: {
        voiceTypingEnabled: false,
        voiceTypingShortcut: 'Alt+V',
        voiceTypingMode: 'hold',
        streamingModelPath: 'path/to/model',
        language: 'auto',
        enableITN: true,
        microphoneId: 'default',
    } as Record<string, any>,
    configSubscribe: vi.fn(),
    invoke: vi.fn(),
    mockPrepare: vi.fn(),
    mockStart: vi.fn(),
    mockSoftStop: vi.fn(),
    setModelPath: vi.fn(),
    setLanguage: vi.fn(),
    setEnableITN: vi.fn(),
    windowPrepare: vi.fn(),
    windowOpen: vi.fn(),
    windowClose: vi.fn(),
    windowSendState: vi.fn(),
    windowClearState: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
    register: vi.fn(),
    unregister: vi.fn(),
    isRegistered: vi.fn(),
}));

vi.mock('../voiceTypingWindowService', () => ({
    voiceTypingWindowService: {
        prepare: mocks.windowPrepare,
        open: mocks.windowOpen,
        close: mocks.windowClose,
        sendState: mocks.windowSendState,
        clearState: mocks.windowClearState,
    },
}));

vi.mock('../transcriptionService', () => {
    class MockTranscriptionService {
        setModelPath = mocks.setModelPath;
        setLanguage = mocks.setLanguage;
        setEnableITN = mocks.setEnableITN;
        prepare = mocks.mockPrepare;
        start = mocks.mockStart;
        softStop = mocks.mockSoftStop;
        stop = vi.fn();
    }

    return {
        TranscriptionService: MockTranscriptionService,
    };
});

vi.mock('../../stores/configStore', () => ({
    useConfigStore: {
        getState: vi.fn(() => ({
            config: mocks.config,
        })),
        subscribe: mocks.configSubscribe,
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

async function loadService() {
    const module = await import('../voiceTypingService');
    return module.voiceTypingService as any;
}

describe('voiceTypingService', () => {
    beforeEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
        vi.resetModules();

        mocks.config = { ...mocks.defaultConfig };
        mocks.configSubscribe.mockImplementation(() => () => undefined);
        mocks.mockPrepare.mockResolvedValue(undefined);
        mocks.mockStart.mockResolvedValue(undefined);
        mocks.mockSoftStop.mockResolvedValue(undefined);
        mocks.windowPrepare.mockResolvedValue(undefined);
        mocks.windowOpen.mockResolvedValue(undefined);
        mocks.windowClose.mockResolvedValue(undefined);
        mocks.windowSendState.mockResolvedValue(undefined);
        mocks.windowClearState.mockResolvedValue(undefined);
        mocks.invoke.mockImplementation(async (command: string) => {
            if (command === 'get_text_cursor_position') {
                return [120, 280];
            }
            if (command === 'get_mouse_position') {
                return [240, 320];
            }
            if (
                command === 'start_microphone_capture' ||
                command === 'inject_text' ||
                command === 'stop_microphone_capture'
            ) {
                return undefined;
            }
            return undefined;
        });
    });

    it('pre-warms the model and overlay window during initialization if enabled', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
            language: 'zh',
        };

        const service = await loadService();
        service.init();
        await Promise.resolve();
        await Promise.resolve();

        expect(mocks.mockPrepare).toHaveBeenCalled();
        expect(mocks.windowPrepare).toHaveBeenCalledWith([0, 0]);
    });

    it('anchors the overlay to the text cursor and publishes preparing then listening before capture stays warm', async () => {
        const callOrder: string[] = [];
        mocks.mockStart.mockImplementation(async () => {
            callOrder.push('start');
        });
        mocks.invoke.mockImplementation(async (command: string) => {
            if (command === 'get_text_cursor_position') {
                callOrder.push('get_text_cursor_position');
                return [120, 280];
            }
            if (command === 'start_microphone_capture') {
                callOrder.push('start_microphone_capture');
                return undefined;
            }
            return undefined;
        });

        const service = await loadService();
        await service.startListening();

        expect(mocks.windowOpen).toHaveBeenCalledWith(116, 288);
        expect(callOrder.indexOf('start_microphone_capture')).toBeGreaterThan(
            callOrder.indexOf('start')
        );
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            1,
            expect.objectContaining({
                sessionId: 'voice-typing-1',
                phase: 'preparing',
                text: '',
                revision: 1,
            })
        );
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            2,
            expect.objectContaining({
                sessionId: 'voice-typing-1',
                phase: 'listening',
                text: '',
                revision: 2,
            })
        );
    });

    it('does not let a late listening placeholder overwrite an already visible segment', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        let resolveStart: (() => void) | undefined;
        mocks.mockStart.mockImplementation((segmentCallback: (segment: any) => Promise<void>) => {
            onSegment = segmentCallback;
            return new Promise<void>((resolve) => {
                resolveStart = resolve;
            });
        });

        const service = await loadService();
        const startPromise = service.startListening();
        await Promise.resolve();

        await onSegment?.({ id: 'seg-early', text: '提早预览', isFinal: false });
        resolveStart?.();
        await startPromise;

        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
            'preparing',
            'segment',
        ]);
    });

    it('refreshes the candidate bar with sentence updates and injects the final transcript', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => Promise<void>) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        await onSegment?.({ id: 'seg-1', text: '你好世', isFinal: false });
        await onSegment?.({ id: 'seg-1', text: '你好世界', isFinal: true });

        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            1,
            expect.objectContaining({
                sessionId: 'voice-typing-1',
                phase: 'segment',
                text: '你好世',
                segmentId: 'seg-1',
                isFinal: false,
                revision: 3,
            })
        );
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            2,
            expect.objectContaining({
                sessionId: 'voice-typing-1',
                phase: 'segment',
                text: '你好世界',
                segmentId: 'seg-1',
                isFinal: true,
                revision: 4,
            })
        );
        expect(mocks.invoke).toHaveBeenCalledWith('inject_text', { text: '你好世界' });
    });

    it('keeps the overlay open in toggle mode and returns to listening after a final segment', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        vi.useFakeTimers();
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
            voiceTypingMode: 'toggle',
        };
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => Promise<void>) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        await onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });

        expect(mocks.invoke).toHaveBeenCalledWith('inject_text', { text: '第一句' });
        expect(mocks.windowClose).not.toHaveBeenCalled();
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            1,
            expect.objectContaining({
                phase: 'segment',
                text: '第一句',
                segmentId: 'seg-1',
                isFinal: true,
                revision: 3,
            })
        );

        await vi.runAllTimersAsync();

        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            2,
            expect.objectContaining({
                phase: 'listening',
                text: '',
                revision: 4,
            })
        );
    });

    it('cancels a pending toggle reset when a newer segment arrives', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        vi.useFakeTimers();
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
            voiceTypingMode: 'toggle',
        };
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => Promise<void>) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        await onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });
        await onSegment?.({ id: 'seg-2', text: '下一句草稿', isFinal: false });
        await vi.runAllTimersAsync();

        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            1,
            expect.objectContaining({
                phase: 'segment',
                text: '第一句',
                segmentId: 'seg-1',
                isFinal: true,
            })
        );
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            2,
            expect.objectContaining({
                phase: 'segment',
                text: '下一句草稿',
                segmentId: 'seg-2',
                isFinal: false,
            })
        );
        expect(
            mocks.windowSendState.mock.calls.some(
                ([payload]) =>
                    payload.phase === 'listening' && payload.revision > 4
            )
        ).toBe(false);
    });

    it('waits for a flush-only final result to stay visible before closing in hold mode', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        let resolveSoftStop: (() => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => Promise<void>) => {
            onSegment = segmentCallback;
        });
        mocks.mockSoftStop.mockImplementation(
            () =>
                new Promise<void>((resolve) => {
                    resolveSoftStop = resolve;
                })
        );

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        const stopPromise = service.stopListening();
        await Promise.resolve();

        expect(mocks.windowClose).not.toHaveBeenCalled();

        await onSegment?.({ id: 'seg-1', text: '短句结果', isFinal: true });

        expect(mocks.windowSendState).toHaveBeenCalledWith(
            expect.objectContaining({
                phase: 'segment',
                text: '短句结果',
                segmentId: 'seg-1',
                isFinal: true,
                revision: 3,
            })
        );
        expect(mocks.invoke).toHaveBeenCalledWith('inject_text', { text: '短句结果' });
        expect(mocks.windowClose).not.toHaveBeenCalled();

        resolveSoftStop?.();
        await vi.advanceTimersByTimeAsync(699);
        expect(mocks.windowClose).not.toHaveBeenCalled();

        await vi.advanceTimersByTimeAsync(1);
        await stopPromise;

        expect(mocks.windowClose).toHaveBeenCalled();
        expect(mocks.windowClearState).toHaveBeenCalled();
    });

    it('ignores stale segment updates after the session is closed', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => Promise<void>) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        await service.stopListening();
        vi.clearAllMocks();

        await onSegment?.({ id: 'seg-stale', text: '过期结果', isFinal: false });

        expect(mocks.windowSendState).not.toHaveBeenCalled();
        expect(mocks.windowOpen).not.toHaveBeenCalled();
    });

    it('flushes the recognizer before closing the voice typing session', async () => {
        vi.useFakeTimers();
        const service = await loadService();

        await service.startListening();
        vi.clearAllMocks();

        const stopPromise = service.stopListening();
        await vi.runAllTimersAsync();
        await stopPromise;

        expect(mocks.invoke).not.toHaveBeenCalledWith(
            'stop_microphone_capture',
            expect.anything()
        );
        expect(mocks.mockSoftStop).toHaveBeenCalled();
        expect(mocks.windowClose).toHaveBeenCalled();
    });

    it('cancels startup cleanly if the shortcut is released before microphone capture begins', async () => {
        let resolveStart: (() => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(
            () =>
                new Promise<void>((resolve) => {
                    resolveStart = resolve;
                })
        );

        const service = await loadService();
        const startPromise = service.startListening();
        await Promise.resolve();

        const stopPromise = service.stopListening();
        resolveStart?.();
        await startPromise;
        await vi.runAllTimersAsync();
        await stopPromise;

        expect(mocks.invoke).not.toHaveBeenCalledWith(
            'start_microphone_capture',
            expect.anything()
        );
        expect(mocks.mockSoftStop).toHaveBeenCalled();
    });
});
