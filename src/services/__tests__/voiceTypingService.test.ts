import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
    const shortcutState: { handler?: (event: any) => void } = {};
    const configEvents: {
        listener?: (state: { config: Record<string, any> }) => void;
    } = {};

    return {
        shortcutState,
        configEvents,
        defaultConfig: {
            voiceTypingEnabled: false,
            voiceTypingShortcut: 'Alt+V',
            voiceTypingMode: 'hold',
            streamingModelPath: 'path/to/model',
            vadModelPath: 'path/to/vad',
            language: 'auto',
            enableITN: true,
            microphoneId: 'default',
        },
        config: {
            voiceTypingEnabled: false,
            voiceTypingShortcut: 'Alt+V',
            voiceTypingMode: 'hold',
            streamingModelPath: 'path/to/model',
            vadModelPath: 'path/to/vad',
            language: 'auto',
            enableITN: true,
            microphoneId: 'default',
        } as Record<string, any>,
        configSubscribe: vi.fn(),
        invoke: vi.fn(),
        register: vi.fn(async (_shortcut: string, handler: (event: any) => void) => {
            shortcutState.handler = handler;
        }),
        unregister: vi.fn(),
        isRegistered: vi.fn().mockResolvedValue(false),
        mockPrepare: vi.fn(),
        mockStart: vi.fn(),
        mockSoftStop: vi.fn(),
        setModelPath: vi.fn(),
        setLanguage: vi.fn(),
        setEnableITN: vi.fn(),
        loggerInfo: vi.fn(),
        loggerWarn: vi.fn(),
        loggerError: vi.fn(),
        loggerDebug: vi.fn(),
        windowPrepare: vi.fn(),
        windowOpen: vi.fn(),
        windowClose: vi.fn(),
        windowSendState: vi.fn(),
        windowClearState: vi.fn(),
    };
});

vi.mock('@tauri-apps/api/core', () => ({
    invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
    register: mocks.register,
    unregister: mocks.unregister,
    isRegistered: mocks.isRegistered,
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
        info: mocks.loggerInfo,
        warn: mocks.loggerWarn,
        error: mocks.loggerError,
        debug: mocks.loggerDebug,
    },
}));

async function loadService() {
    const module = await import('../voiceTypingService');
    return module.voiceTypingService as any;
}

async function loadRuntimeStore() {
    return await import('../../stores/voiceTypingRuntimeStore');
}

async function flushMicrotasks(times = 4) {
    for (let i = 0; i < times; i += 1) {
        await Promise.resolve();
    }
}

function getInvokeCalls(command: string) {
    return mocks.invoke.mock.calls.filter(([calledCommand]) => calledCommand === command);
}

function emitConfigChange(patch: Record<string, any>) {
    mocks.config = {
        ...mocks.config,
        ...patch,
    };
    mocks.configEvents.listener?.({ config: mocks.config });
}

describe('voiceTypingService', () => {
    beforeEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
        vi.resetModules();

        mocks.shortcutState.handler = undefined;
        mocks.configEvents.listener = undefined;
        mocks.config = { ...mocks.defaultConfig };
        mocks.configSubscribe.mockImplementation((listener: (state: { config: Record<string, any> }) => void) => {
            mocks.configEvents.listener = listener;
            return () => undefined;
        });
        mocks.isRegistered.mockResolvedValue(false);
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
        await flushMicrotasks();

        expect(mocks.mockPrepare).toHaveBeenCalled();
        expect(mocks.windowPrepare).toHaveBeenCalledWith([0, 0]);
    });

    it('updates runtime status to ready after successful shortcut registration and warm-up', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
        };

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        service.init();
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                shortcutRegistration: 'ready',
                warmup: 'ready',
                lastErrorSource: null,
                lastErrorMessage: null,
            })
        );
    });

    it('records shortcut registration failures in the runtime status store', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
        };
        mocks.register.mockRejectedValueOnce(new Error('Shortcut is already in use.'));

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        service.init();
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                shortcutRegistration: 'error',
                lastErrorSource: 'shortcut_registration',
                lastErrorMessage: 'Shortcut is already in use.',
            })
        );
    });

    it('records warm-up failures in the runtime status store', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
        };
        mocks.mockPrepare.mockRejectedValueOnce(new Error('Warm-up failed.'));

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        service.init();
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                warmup: 'error',
                lastErrorSource: 'warmup',
                lastErrorMessage: 'Warm-up failed.',
            })
        );
    });

    it('records microphone pre-warm failures in the runtime status store', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
        };
        mocks.invoke.mockImplementation(async (command: string) => {
            if (command === 'get_text_cursor_position') {
                return [120, 280];
            }
            if (command === 'get_mouse_position') {
                return [240, 320];
            }
            if (command === 'start_microphone_capture') {
                throw new Error('Microphone is unavailable.');
            }
            if (command === 'inject_text' || command === 'stop_microphone_capture') {
                return undefined;
            }
            return undefined;
        });

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        service.init();
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                warmup: 'error',
                lastErrorSource: 'microphone',
                lastErrorMessage: 'Microphone is unavailable.',
            })
        );
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
        let onSegment: ((segment: any) => void) | undefined;
        let resolveStart: (() => void) | undefined;
        mocks.mockStart.mockImplementation((segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
            return new Promise<void>((resolve) => {
                resolveStart = resolve;
            });
        });

        const service = await loadService();
        const startPromise = service.startListening();
        await flushMicrotasks(2);

        onSegment?.({ id: 'seg-early', text: '提早预览', isFinal: false });
        await flushMicrotasks();
        resolveStart?.();
        await startPromise;

        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
            'preparing',
            'segment',
        ]);
    });

    it('keeps an existing candidate visible when later partials collapse to tags or empty text', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-1', text: '测试123', isFinal: false });
        await flushMicrotasks(8);
        onSegment?.({ id: 'seg-1', text: '<|zh|><|withitn|>', isFinal: false });
        await flushMicrotasks(8);
        onSegment?.({ id: 'seg-1', text: '   ', isFinal: false });
        await flushMicrotasks(8);

        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
            'segment',
        ]);
        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.text)).toEqual([
            '测试123',
        ]);
        expect(mocks.loggerInfo).toHaveBeenCalledWith(
            '[VoiceTypingSessionMachine] Dropped segment update',
            expect.objectContaining({
                dropReason: 'tag_only',
            })
        );
        expect(mocks.loggerInfo).toHaveBeenCalledWith(
            '[VoiceTypingSessionMachine] Dropped segment update',
            expect.objectContaining({
                dropReason: 'empty_after_normalize',
            })
        );
    });

    it('allows punctuation-only partial updates to appear as candidate text', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-1', text: '。', isFinal: false });
        await flushMicrotasks(8);

        expect(mocks.windowSendState).toHaveBeenCalledWith(
            expect.objectContaining({
                phase: 'segment',
                text: '。',
                segmentId: 'seg-1',
                isFinal: false,
            })
        );
    });

    it('keeps publishing later partial updates for the same sentence instead of only the first draft', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-1', text: '喂。', isFinal: false });
        await flushMicrotasks(8);
        onSegment?.({ id: 'seg-1', text: '喂，123。', isFinal: false });
        await flushMicrotasks(8);
        onSegment?.({ id: 'seg-1', text: '喂，123，继续。', isFinal: false });
        await flushMicrotasks(8);

        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
            'segment',
            'segment',
            'segment',
        ]);
        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.text)).toEqual([
            '喂。',
            '喂，123。',
            '喂，123，继续。',
        ]);
        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.revision)).toEqual([
            3,
            4,
            5,
        ]);
    });

    it('drops non-final partial updates after manual stop is requested', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        let resolveSoftStop: (() => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
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
        await flushMicrotasks(2);

        onSegment?.({ id: 'seg-1', text: '停止后的草稿', isFinal: false });
        await flushMicrotasks(8);

        expect(mocks.windowSendState).not.toHaveBeenCalled();
        expect(mocks.loggerInfo).toHaveBeenCalledWith(
            '[VoiceTypingSessionMachine] Dropped segment update',
            expect.objectContaining({
                dropReason: 'manual_stop_pending',
                segmentId: 'seg-1',
            })
        );

        resolveSoftStop?.();
        await vi.runAllTimersAsync();
        await stopPromise;
    });

    it('commits a finalized sentence and repositions the candidate bar from the updated caret', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        let cursorCallCount = 0;
        mocks.invoke.mockImplementation(async (command: string) => {
            if (command === 'get_text_cursor_position') {
                cursorCallCount += 1;
                return cursorCallCount === 1 ? [120, 280] : [160, 340];
            }
            if (
                command === 'start_microphone_capture' ||
                command === 'inject_text'
            ) {
                return undefined;
            }
            if (command === 'get_mouse_position') {
                return [240, 320];
            }
            return undefined;
        });
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-1', text: '你好世', isFinal: false });
        await flushMicrotasks(8);
        onSegment?.({ id: 'seg-1', text: '你好世界', isFinal: true });
        await flushMicrotasks(12);

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
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            3,
            expect.objectContaining({
                sessionId: 'voice-typing-1',
                phase: 'listening',
                text: '',
                revision: 5,
            })
        );
        expect(getInvokeCalls('inject_text')).toEqual([[
            'inject_text',
            { text: '你好世界', shortcutModifiers: ['alt'] },
        ]]);
        expect(mocks.windowOpen).toHaveBeenCalledWith(156, 348);
    });

    it('retries caret lookup after commit until the cursor position actually moves', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        let cursorCallCount = 0;
        vi.useFakeTimers();
        mocks.invoke.mockImplementation(async (command: string) => {
            if (command === 'get_text_cursor_position') {
                cursorCallCount += 1;
                if (cursorCallCount <= 3) {
                    return [120, 280];
                }
                return [180, 360];
            }
            if (
                command === 'start_microphone_capture' ||
                command === 'inject_text'
            ) {
                return undefined;
            }
            if (command === 'get_mouse_position') {
                return [240, 320];
            }
            return undefined;
        });
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });
        await flushMicrotasks(8);
        await vi.advanceTimersByTimeAsync(39);
        expect(mocks.windowOpen).not.toHaveBeenCalledWith(176, 368);
        await vi.advanceTimersByTimeAsync(1);
        await flushMicrotasks(8);
        await vi.advanceTimersByTimeAsync(40);
        await flushMicrotasks(8);

        expect(getInvokeCalls('inject_text')).toEqual([[
            'inject_text',
            { text: '第一句', shortcutModifiers: ['alt'] },
        ]]);
        expect(mocks.windowOpen).toHaveBeenCalledWith(176, 368);
    });

    it('passes control modifiers to inject_text when the voice typing shortcut uses Ctrl + Space', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingShortcut: 'Ctrl + Space',
        };
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-ctrl', text: '123。', isFinal: true });
        await flushMicrotasks(12);

        expect(getInvokeCalls('inject_text')).toEqual([[
            'inject_text',
            { text: '123。', shortcutModifiers: ['control'] },
        ]]);
    });

    it('omits shortcut modifiers when the voice typing shortcut has no modifier keys', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingShortcut: 'Space',
        };
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-space', text: '纯文本', isFinal: true });
        await flushMicrotasks(12);

        expect(getInvokeCalls('inject_text')).toEqual([[
            'inject_text',
            { text: '纯文本' },
        ]]);
    });

    it('keeps the session alive across VAD sentence boundaries in hold mode', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });
        await flushMicrotasks(8);
        await vi.advanceTimersByTimeAsync(120);
        await flushMicrotasks(8);
        onSegment?.({ id: 'seg-2', text: '第二句草稿', isFinal: false });
        await flushMicrotasks(8);

        expect(getInvokeCalls('inject_text')).toEqual([[
            'inject_text',
            { text: '第一句', shortcutModifiers: ['alt'] },
        ]]);
        expect(mocks.windowClose).not.toHaveBeenCalled();
        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
            'segment',
            'listening',
            'segment',
        ]);
        expect(
            mocks.windowSendState.mock.calls.map(([payload]) => payload.text)
        ).toEqual(['第一句', '', '第二句草稿']);
    });

    it('keeps overlay revisions monotonic after a session is stopped and restarted', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        onSegment?.({ id: 'seg-1', text: '第一轮候选', isFinal: false });
        await flushMicrotasks(8);

        const firstStopPromise = service.stopListening();
        await vi.runAllTimersAsync();
        await firstStopPromise;

        vi.clearAllMocks();

        await service.startListening();
        onSegment?.({ id: 'seg-2', text: '第二轮候选', isFinal: false });
        await flushMicrotasks(8);

        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            1,
            expect.objectContaining({
                sessionId: 'voice-typing-2',
                phase: 'preparing',
                revision: 4,
            })
        );
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            2,
            expect.objectContaining({
                sessionId: 'voice-typing-2',
                phase: 'listening',
                revision: 5,
            })
        );
        expect(mocks.windowSendState).toHaveBeenNthCalledWith(
            3,
            expect.objectContaining({
                sessionId: 'voice-typing-2',
                phase: 'segment',
                text: '第二轮候选',
                revision: 6,
            })
        );
    });

    it('uses pressed events to toggle start and stop in toggle mode', async () => {
        vi.useFakeTimers();
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
            voiceTypingMode: 'toggle',
        };

        const service = await loadService();
        service.init();
        await flushMicrotasks();

        expect(mocks.register).toHaveBeenCalled();
        expect(mocks.shortcutState.handler).toBeTruthy();

        mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Pressed' });
        await flushMicrotasks();

        expect(mocks.mockStart).toHaveBeenCalledTimes(1);

        mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Released' });
        await flushMicrotasks();

        expect(mocks.mockSoftStop).not.toHaveBeenCalled();

        mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Pressed' });
        await vi.runAllTimersAsync();

        expect(mocks.mockSoftStop).toHaveBeenCalledTimes(1);
    });

    it('closes only after a flush-only final sentence is committed during manual stop', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        let resolveSoftStop: (() => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
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
        await flushMicrotasks(2);

        expect(mocks.windowClose).not.toHaveBeenCalled();

        onSegment?.({ id: 'seg-1', text: '短句结果', isFinal: true });
        await flushMicrotasks(8);

        expect(getInvokeCalls('inject_text')).toEqual([[
            'inject_text',
            { text: '短句结果', shortcutModifiers: ['alt'] },
        ]]);
        expect(mocks.windowSendState).toHaveBeenCalledTimes(1);
        expect(mocks.windowSendState).toHaveBeenCalledWith(
            expect.objectContaining({
                phase: 'segment',
                text: '短句结果',
                segmentId: 'seg-1',
                isFinal: true,
                revision: 3,
            })
        );
        expect(mocks.windowClose).not.toHaveBeenCalled();

        resolveSoftStop?.();
        await vi.advanceTimersByTimeAsync(79);
        expect(mocks.windowClose).not.toHaveBeenCalled();

        await vi.advanceTimersByTimeAsync(1);
        await stopPromise;

        expect(mocks.windowClose).toHaveBeenCalledTimes(1);
        expect(mocks.windowClearState).toHaveBeenCalledTimes(1);
    });

    it('ignores duplicate finals for a sentence that was already committed', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-1', text: '重复句子', isFinal: true });
        await flushMicrotasks(8);
        await vi.advanceTimersByTimeAsync(120);
        await flushMicrotasks(8);
        onSegment?.({ id: 'seg-1', text: '重复句子', isFinal: true });
        await flushMicrotasks(8);

        expect(getInvokeCalls('inject_text')).toEqual([[
            'inject_text',
            { text: '重复句子', shortcutModifiers: ['alt'] },
        ]]);
        expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
            'segment',
            'listening',
        ]);
    });

    it('ignores stale segment updates after the session is closed', async () => {
        let onSegment: ((segment: any) => void) | undefined;
        vi.useFakeTimers();
        mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
            onSegment = segmentCallback;
        });

        const service = await loadService();
        await service.startListening();
        const stopPromise = service.stopListening();
        await vi.runAllTimersAsync();
        await stopPromise;
        vi.clearAllMocks();

        onSegment?.({ id: 'seg-stale', text: '过期结果', isFinal: false });
        await flushMicrotasks();

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

    it('propagates session errors into the runtime status store', async () => {
        mocks.mockStart.mockImplementation(
            async (
                _segmentCallback: (segment: any) => void,
                errorCallback: (error: string) => void
            ) => {
                errorCallback('Recognizer crashed.');
            }
        );

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        await service.startListening();
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                lastErrorSource: 'session',
                lastErrorMessage: 'Recognizer crashed.',
            })
        );
    });

    it('clears stale shortcut errors after the shortcut changes and registration recovers', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
        };
        mocks.register.mockRejectedValueOnce(new Error('Shortcut conflict.'));

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        service.init();
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                shortcutRegistration: 'error',
                lastErrorSource: 'shortcut_registration',
            })
        );

        emitConfigChange({ voiceTypingShortcut: 'Ctrl + Shift + V' });
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                shortcutRegistration: 'ready',
                warmup: 'ready',
                lastErrorSource: null,
                lastErrorMessage: null,
            })
        );
    });

    it('clears stale warm-up errors after a dependency change triggers recovery', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
        };
        mocks.mockPrepare.mockRejectedValueOnce(new Error('Warm-up failed once.'));

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        service.init();
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                warmup: 'error',
                lastErrorSource: 'warmup',
            })
        );

        emitConfigChange({ microphoneId: 'usb-mic' });
        await flushMicrotasks(8);

        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                shortcutRegistration: 'ready',
                warmup: 'ready',
                lastErrorSource: null,
                lastErrorMessage: null,
            })
        );
    });

    it('retries warm-up explicitly for diagnostics and restores runtime readiness', async () => {
        mocks.config = {
            ...mocks.defaultConfig,
            voiceTypingEnabled: true,
        };

        const service = await loadService();
        const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

        service.init();
        await flushMicrotasks(8);

        useVoiceTypingRuntimeStore.getState().reportRuntimeError('warmup', 'Warm-up failed once.');
        vi.clearAllMocks();

        await service.retryWarmup();
        await flushMicrotasks(8);

        expect(getInvokeCalls('stop_microphone_capture')).toEqual([[
            'stop_microphone_capture',
            { instanceId: 'voice-typing' },
        ]]);
        expect(getInvokeCalls('start_microphone_capture')).toEqual([[
            'start_microphone_capture',
            {
                deviceName: null,
                instanceId: 'voice-typing',
            },
        ]]);
        expect(mocks.mockPrepare).toHaveBeenCalled();
        expect(useVoiceTypingRuntimeStore.getState()).toEqual(
            expect.objectContaining({
                shortcutRegistration: 'ready',
                warmup: 'ready',
                lastErrorSource: null,
                lastErrorMessage: null,
            })
        );
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
        await flushMicrotasks(2);

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
