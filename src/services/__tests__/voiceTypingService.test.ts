import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { voiceTypingService } from '../voiceTypingService';
import { voiceTypingWindowService } from '../voiceTypingWindowService';

const { mockPrepare, mockStart, mockSoftStop } = vi.hoisted(() => ({
    mockPrepare: vi.fn(),
    mockStart: vi.fn(),
    mockSoftStop: vi.fn(),
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
        open: vi.fn(),
        close: vi.fn(),
        sendText: vi.fn(),
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

vi.mock('../utils/logger', () => ({
    logger: {
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
        debug: vi.fn(),
    },
}));

describe('voiceTypingService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockPrepare.mockResolvedValue(undefined);
        mockSoftStop.mockResolvedValue(undefined);
        mockStart.mockResolvedValue(undefined);

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
    });

    it('pre-warms the model during initialization if enabled', async () => {
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

        expect(mockPrepare).toHaveBeenCalled();
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

        expect(voiceTypingWindowService.open).toHaveBeenCalledWith(120, 292);
        expect(callOrder.indexOf('start')).toBeGreaterThan(callOrder.indexOf('prepare'));
        expect(callOrder.indexOf('start_microphone_capture')).toBeGreaterThan(callOrder.indexOf('start'));
    });

    it('shows partial and final text, and injects the final transcript', async () => {
        let onSegment: ((segment: any) => Promise<void>) | undefined;

        mockStart.mockImplementation(async (segmentCallback) => {
            onSegment = segmentCallback;
        });

        await (voiceTypingService as any).startListening();

        await onSegment?.({ text: '你好世', isFinal: false });
        await onSegment?.({ text: '你好世界', isFinal: true });

        expect(voiceTypingWindowService.sendText).toHaveBeenCalledWith('你好世');
        expect(voiceTypingWindowService.sendText).toHaveBeenCalledWith('你好世界');
        expect(voiceTypingWindowService.sendText).not.toHaveBeenCalledWith('正在聆听...');
        expect(invoke).toHaveBeenCalledWith('inject_text', { text: '你好世界' });
    });

    it('flushes the recognizer before closing the voice typing session', async () => {
        await (voiceTypingService as any).startListening();

        await (voiceTypingService as any).stopListening();

        expect(invoke).toHaveBeenCalledWith('stop_microphone_capture', { instanceId: 'voice-typing' });
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
