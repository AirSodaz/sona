import { renderHook, act, waitFor } from '@testing-library/react';
import { useCaptionSession } from '../useCaptionSession';
import { captionWindowService } from '../../services/captionWindowService';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { buildTestConfig } from '../../test-utils/configTestUtils';

const tauriCoreMocks = vi.hoisted(() => ({
    invoke: vi.fn(),
}));

const tauriEventMocks = vi.hoisted(() => ({
    listen: vi.fn(),
}));

const tauriFsMocks = vi.hoisted(() => ({
    remove: vi.fn(),
}));

const effectiveConfigMocks = vi.hoisted(() => ({
    config: null as ReturnType<typeof buildTestConfig> | null,
}));

vi.mock('@tauri-apps/api/core', () => ({
    invoke: tauriCoreMocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: tauriEventMocks.listen,
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    remove: tauriFsMocks.remove,
}));

vi.mock('../../services/captionWindowService', () => ({
    captionWindowService: {
        open: vi.fn().mockResolvedValue(undefined),
        close: vi.fn().mockResolvedValue(undefined),
        sendSegments: vi.fn().mockResolvedValue(undefined),
        updateStyle: vi.fn().mockResolvedValue(undefined),
    }
}));

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [],
    PRESET_MODELS_MAP: new Map(),
    modelService: {
        getEnabledITNModelPaths: vi.fn().mockResolvedValue([]),
        getModelRules: vi.fn(() => ({
            requiresPunctuation: false,
            requiresVad: false,
        })),
    }
}));

vi.mock('../../stores/effectiveConfigStore', () => ({
    getEffectiveConfigSnapshot: vi.fn(() => effectiveConfigMocks.config),
}));

const transcriptionServiceMocks = vi.hoisted(() => ({
    recordStart: vi.fn(),
    recordStop: vi.fn(),
    recordSendAudioInt16: vi.fn(),
    captionStart: vi.fn(),
    captionStop: vi.fn(),
    captionSendAudioInt16: vi.fn(),
}));

vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        start: transcriptionServiceMocks.recordStart,
        stop: transcriptionServiceMocks.recordStop,
        sendAudioInt16: transcriptionServiceMocks.recordSendAudioInt16,
    },
    captionTranscriptionService: {
        start: transcriptionServiceMocks.captionStart,
        stop: transcriptionServiceMocks.captionStop,
        sendAudioInt16: transcriptionServiceMocks.captionSendAudioInt16,
    }
}));

const audioContextMocks = vi.hoisted(() => ({
    close: vi.fn().mockResolvedValue(undefined),
    resume: vi.fn().mockResolvedValue(undefined),
    addModule: vi.fn().mockResolvedValue(undefined),
}));

vi.stubGlobal('AudioContext', class {
    state = 'running';
    destination = {};
    audioWorklet = {
        addModule: audioContextMocks.addModule,
    };
    createMediaStreamSource() {
        return { connect: vi.fn() };
    }
    close = audioContextMocks.close;
    resume = audioContextMocks.resume;
});

vi.stubGlobal('AudioWorkletNode', class {
    port = { onmessage: null };
    connect = vi.fn();
});

vi.stubGlobal('MediaStream', class {
    tracks: any[];
    constructor(tracks?: any[]) {
        this.tracks = tracks || [];
    }
    getAudioTracks() { return this.tracks; }
    getVideoTracks() { return []; }
    getTracks() { return this.tracks; }
});

describe('useCaptionSession', () => {
    let mockStream: any;
    const flushMicrotasks = async () => {
        await act(async () => {
            await Promise.resolve();
        });
    };

    const defaultConfig = buildTestConfig({
        streamingModelPath: '/path/to/model',
        offlineModelPath: '/path/to/model',
        language: 'en',
    });

    beforeEach(() => {
        vi.clearAllMocks();
        effectiveConfigMocks.config = defaultConfig;

        mockStream = {
            getAudioTracks: () => [{ onended: null, stop: vi.fn() }],
            getVideoTracks: () => [{ stop: vi.fn() }],
            getTracks: () => [{ stop: vi.fn() }],
        };

        transcriptionServiceMocks.captionStart.mockResolvedValue(undefined);
        transcriptionServiceMocks.captionStop.mockResolvedValue(undefined);
        transcriptionServiceMocks.recordStart.mockResolvedValue(undefined);
        transcriptionServiceMocks.recordStop.mockResolvedValue(undefined);
        tauriCoreMocks.invoke.mockRejectedValue(new Error('Native capture not supported'));
        tauriEventMocks.listen.mockResolvedValue(vi.fn());
        tauriFsMocks.remove.mockResolvedValue(undefined);

        if (!navigator.mediaDevices) {
            Object.defineProperty(navigator, 'mediaDevices', {
                value: {},
                writable: true,
            });
        }

        navigator.mediaDevices.getDisplayMedia = vi.fn().mockResolvedValue(mockStream);
    });

    it('uses captionTranscriptionService for caption lifecycle and cleanup', async () => {
        const { rerender } = renderHook(
            (props) => useCaptionSession(props.config, props.isCaptionMode),
            { initialProps: { config: defaultConfig, isCaptionMode: true } }
        );

        await waitFor(() => expect(transcriptionServiceMocks.captionStart).toHaveBeenCalled());

        expect(transcriptionServiceMocks.recordStart).not.toHaveBeenCalled();

        rerender({ config: defaultConfig, isCaptionMode: false });

        await waitFor(() => expect(transcriptionServiceMocks.captionStop).toHaveBeenCalled());

        expect(transcriptionServiceMocks.recordStop).not.toHaveBeenCalled();
    });

    it('does not touch the record service when caption is toggled off during initialization', async () => {
        let resolveDisplayMedia: (value: any) => void = () => { };

        navigator.mediaDevices.getDisplayMedia = vi.fn().mockImplementation(() => {
            return new Promise((resolve) => {
                resolveDisplayMedia = resolve;
            });
        });

        const { rerender, unmount } = renderHook(
            (props) => useCaptionSession(props.config, props.isCaptionMode),
            { initialProps: { config: defaultConfig, isCaptionMode: true } }
        );

        await waitFor(() => expect(navigator.mediaDevices.getDisplayMedia).toHaveBeenCalled());

        await act(async () => {
            rerender({ config: defaultConfig, isCaptionMode: false });
            await Promise.resolve();
        });

        await waitFor(() => expect(captionWindowService.close).toHaveBeenCalled());

        await act(async () => {
            resolveDisplayMedia(mockStream);
            await Promise.resolve();
        });
        await flushMicrotasks();

        expect(captionWindowService.open).not.toHaveBeenCalled();
        expect(transcriptionServiceMocks.recordStart).not.toHaveBeenCalled();
        expect(transcriptionServiceMocks.recordStop).not.toHaveBeenCalled();

        unmount();
    });

    it('starts and cleans up native caption capture without display media fallback', async () => {
        const unlisten = vi.fn();
        tauriEventMocks.listen.mockResolvedValue(unlisten);
        tauriCoreMocks.invoke.mockImplementation(async (command: string) => {
            if (command === 'start_system_audio_capture') {
                return undefined;
            }

            if (command === 'stop_system_audio_capture') {
                return 'C:/tmp/caption.wav';
            }

            throw new Error(`Unexpected native command: ${command}`);
        });

        const { rerender } = renderHook(
            (props) => useCaptionSession(props.config, props.isCaptionMode),
            { initialProps: { config: defaultConfig, isCaptionMode: true } }
        );

        await waitFor(() => expect(transcriptionServiceMocks.captionStart).toHaveBeenCalled());

        expect(tauriCoreMocks.invoke).toHaveBeenCalledWith('start_system_audio_capture', {
            deviceName: null,
            instanceId: 'caption',
        });
        expect(navigator.mediaDevices.getDisplayMedia).not.toHaveBeenCalled();
        expect(captionWindowService.open).toHaveBeenCalled();

        rerender({ config: defaultConfig, isCaptionMode: false });

        await waitFor(() => expect(transcriptionServiceMocks.captionStop).toHaveBeenCalled());

        expect(tauriCoreMocks.invoke).toHaveBeenCalledWith('stop_system_audio_capture', {
            instanceId: 'caption',
        });
        expect(tauriFsMocks.remove).toHaveBeenCalledWith('C:/tmp/caption.wav');
        expect(unlisten).toHaveBeenCalledTimes(1);
    });
});
