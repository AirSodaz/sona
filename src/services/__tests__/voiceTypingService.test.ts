import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { voiceTypingService } from '../voiceTypingService';
import { voiceTypingWindowService } from '../voiceTypingWindowService';

const { mockPrepare, mockStart, mockSoftStop, mockWindowPrepare } = vi.hoisted(() => ({
    mockPrepare: vi.fn(),
    mockStart: vi.fn(),
    mockSoftStop: vi.fn(),
    mockWindowPrepare: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
    register: vi.fn(),
    unregister: vi.fn(),
    isRegistered: vi.fn(),
}));

vi.mock('../voiceTypingWindowService', () => ({
    voiceTypingWindowService: {
        prepare: mockWindowPrepare,
        open: vi.fn(),
        close: vi.fn().mockResolvedValue(undefined),
        sendState: vi.fn(),
    },
}));

vi.mock('../transcriptionService', () => {
    class MockTranscriptionService {
        setModelPath = vi.fn();
        setLanguage = vi.fn();
        setEnableITN = vi.fn();
        prepare = mockPrepare;
        start = mockStart;
        softStop = mockSoftStop;
        stop = vi.fn();
    }
    return {
        TranscriptionService: MockTranscriptionService,
        transcriptionService: {},
        captionTranscriptionService: {},
    };
});

vi.mock('../../stores/configStore', () => ({
    useConfigStore: {
        getState: vi.fn(() => ({
            config: {
                voiceTypingEnabled: false,
                voiceTypingShortcut: 'Alt+V',
                streamingModelPath: 'path/to/model',
                language: 'auto',
                enableITN: true
            }
        })),
        subscribe: vi.fn(),
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

describe('voiceTypingService', () => {
    beforeEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
        mockPrepare.mockResolvedValue(undefined);
        mockSoftStop.mockResolvedValue(undefined);
        mockStart.mockResolvedValue(undefined);
        mockWindowPrepare.mockResolvedValue(undefined);

        vi.mocked(invoke).mockImplementation(async (command) => {
            if (command === 'get_text_cursor_position') {
                return [120, 280];
            }
            if (command === 'start_microphone_capture') {
                return undefined;
            }
            if (command === 'inject_text') {
                return undefined;
            }
            if (command === 'stop_microphone_capture') {
                return '';
            }
            return undefined;
        });

        const service = voiceTypingService as any;
        service.isListening = false;
        service.initialized = false;
        service.currentShortcut = null;
        service.captureStarted = false;
        service.lastEnabled = false;
        service.lastShortcut = '';
        service.lastModelPath = '';
        service.lastLanguage = '';
        service.lastEnableITN = true;
        service.startRequestId = 0;
        service.activeSessionId = null;
        service.activeSegmentId = null;
        service.overlayVisible = false;
        service.lastOverlayPosition = null;
        service.lastOverlayPayload = null;
        service.sessionState = 'idle';
        service.listeningResetTimer = null;
    });

    it('pre-warms the model and overlay window during initialization if enabled', async () => {
        const { useConfigStore } = await import('../../stores/configStore');
        vi.mocked(useConfigStore.getState).mockReturnValue({
            config: {
                voiceTypingEnabled: true,
                streamingModelPath: 'path/to/model',
                language: 'zh',
                enableITN: true
            }
        } as any);

        voiceTypingService.init();
        await Promise.resolve();
        await Promise.resolve();

        expect(mockPrepare).toHaveBeenCalled();
        expect(mockWindowPrepare).toHaveBeenCalledWith([0, 0]);
    });

    it('anchors the overlay to the text cursor and starts recognition before microphone capture', async () => {
        const callOrder: string[] = [];

        mockPrepare.mockImplementation(async () => {
            callOrder.push('prepare');
        });
        mockStart.mockImplementation(async () => {
            callOrder.push('start');
        });
        vi.mocked(invoke).mockImplementation(async (command) => {
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

        await (voiceTypingService as any).startListening();

        expect(voiceTypingWindowService.open).toHaveBeenCalledWith(116, 288);
        expect(callOrder.indexOf('start')).toBeGreaterThan(callOrder.indexOf('prepare'));
        expect(callOrder.indexOf('start_microphone_capture')).toBeGreaterThan(callOrder.indexOf('start'));
        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'preparing',
            text: '',
        });
    });

    it('refreshes the candidate bar with sentence updates and injects the final transcript', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;

        mockStart.mockImplementation(async (segmentCallback) => {
            onSegment = segmentCallback;
        });

        await (voiceTypingService as any).startListening();

        await onSegment?.({ id: 'seg-1', text: '你好世', isFinal: false });
        await onSegment?.({ id: 'seg-1', text: '你好世界', isFinal: true });

        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'segment',
            text: '你好世',
            segmentId: 'seg-1',
            isFinal: false,
        });
        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'segment',
            text: '你好世界',
            segmentId: 'seg-1',
            isFinal: true,
        });
        expect(invoke).toHaveBeenCalledWith('inject_text', { text: '你好世界' });
    });

    it('keeps the overlay open in toggle mode and returns to listening after a final segment', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        const { useConfigStore } = await import('../../stores/configStore');
        vi.useFakeTimers();

        vi.mocked(useConfigStore.getState).mockReturnValue({
            config: {
                voiceTypingEnabled: true,
                voiceTypingShortcut: 'Alt+V',
                voiceTypingMode: 'toggle',
                streamingModelPath: 'path/to/model',
                language: 'auto',
                enableITN: true,
            }
        } as any);

        mockStart.mockImplementation(async (segmentCallback) => {
            onSegment = segmentCallback;
        });

        await (voiceTypingService as any).startListening();
        vi.clearAllMocks();

        await onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });

        expect(invoke).toHaveBeenCalledWith('inject_text', { text: '第一句' });
        expect(voiceTypingWindowService.close).not.toHaveBeenCalled();
        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'segment',
            text: '第一句',
            segmentId: 'seg-1',
            isFinal: true,
        });

        await vi.runAllTimersAsync();

        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'listening',
            text: '',
        });
    });

    it('waits for flush final output before closing in hold mode', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        let resolveSoftStop: (() => void) | undefined;
        vi.useFakeTimers();

        mockStart.mockImplementation(async (segmentCallback) => {
            onSegment = segmentCallback;
        });
        mockSoftStop.mockImplementation(() => new Promise<void>((resolve) => {
            resolveSoftStop = resolve;
        }));

        await (voiceTypingService as any).startListening();
        vi.clearAllMocks();

        const stopPromise = (voiceTypingService as any).stopListening();
        await Promise.resolve();

        expect(voiceTypingWindowService.close).not.toHaveBeenCalled();
        await onSegment?.({ id: 'seg-1', text: '短句结果', isFinal: true });
        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'segment',
            text: '短句结果',
            segmentId: 'seg-1',
            isFinal: true,
        });
        expect(invoke).toHaveBeenCalledWith('inject_text', { text: '短句结果' });

        resolveSoftStop?.();
        await vi.runAllTimersAsync();
        await stopPromise;

        expect(voiceTypingWindowService.close).toHaveBeenCalled();
        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'listening',
            text: '',
        });
    });

    it('waits for a late final callback dispatched right after flush resolves', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;
        vi.useFakeTimers();

        mockStart.mockImplementation(async (segmentCallback) => {
            onSegment = segmentCallback;
        });
        mockSoftStop.mockResolvedValue(undefined);

        await (voiceTypingService as any).startListening();
        vi.clearAllMocks();

        const stopPromise = (voiceTypingService as any).stopListening();
        await Promise.resolve();

        expect(voiceTypingWindowService.close).not.toHaveBeenCalled();

        await onSegment?.({ id: 'seg-late', text: '迟到最终句', isFinal: true });

        expect(voiceTypingWindowService.sendState).toHaveBeenCalledWith({
            sessionId: 'voice-typing-1',
            phase: 'segment',
            text: '迟到最终句',
            segmentId: 'seg-late',
            isFinal: true,
        });
        expect(invoke).toHaveBeenCalledWith('inject_text', { text: '迟到最终句' });

        await vi.runAllTimersAsync();
        await stopPromise;

        expect(voiceTypingWindowService.close).toHaveBeenCalled();
    });

    it('ignores stale segment updates after the session is closed', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;

        mockStart.mockImplementation(async (segmentCallback) => {
            onSegment = segmentCallback;
        });

        await (voiceTypingService as any).startListening();
        await (voiceTypingService as any).stopListening();
        vi.clearAllMocks();

        await onSegment?.({ id: 'seg-stale', text: '过期结果', isFinal: false });

        expect(voiceTypingWindowService.sendState).not.toHaveBeenCalled();
        expect(voiceTypingWindowService.open).not.toHaveBeenCalled();
    });

    it('flushes the recognizer before closing the voice typing session', async () => {
        await (voiceTypingService as any).startListening();

        await (voiceTypingService as any).stopListening();

        expect(invoke).not.toHaveBeenCalledWith('stop_microphone_capture', expect.anything());
        expect(mockSoftStop).toHaveBeenCalled();
        expect(voiceTypingWindowService.close).toHaveBeenCalled();
    });

    it('cancels startup cleanly if the shortcut is released before microphone capture begins', async () => {
        let resolveStart: (() => void) | undefined;

        mockStart.mockImplementation(() => new Promise<void>((resolve) => {
            resolveStart = resolve;
        }));

        const startPromise = (voiceTypingService as any).startListening();
        await Promise.resolve();

        await (voiceTypingService as any).stopListening();
        resolveStart?.();
        await startPromise;

        expect(invoke).not.toHaveBeenCalledWith('start_microphone_capture', expect.anything());
        expect(mockSoftStop).toHaveBeenCalled();
    });
});
