import { renderHook, act, waitFor } from '@testing-library/react';
import { useCaptionSession } from '../useCaptionSession';
import { captionWindowService } from '../../services/captionWindowService';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { AppConfig } from '../../types/transcript';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockRejectedValue(new Error('Native capture not supported')),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
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
    modelService: {
        getEnabledITNModelPaths: vi.fn().mockResolvedValue([]),
    }
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

    const defaultConfig: AppConfig = {
        streamingModelPath: '/path/to/model',
        offlineModelPath: '/path/to/model',
        language: 'en',
    } as AppConfig;

    beforeEach(() => {
        vi.clearAllMocks();

        mockStream = {
            getAudioTracks: () => [{ onended: null, stop: vi.fn() }],
            getVideoTracks: () => [{ stop: vi.fn() }],
            getTracks: () => [{ stop: vi.fn() }],
        };

        transcriptionServiceMocks.captionStart.mockResolvedValue(undefined);
        transcriptionServiceMocks.captionStop.mockResolvedValue(undefined);
        transcriptionServiceMocks.recordStart.mockResolvedValue(undefined);
        transcriptionServiceMocks.recordStop.mockResolvedValue(undefined);

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

        const { rerender } = renderHook(
            (props) => useCaptionSession(props.config, props.isCaptionMode),
            { initialProps: { config: defaultConfig, isCaptionMode: true } }
        );

        await waitFor(() => expect(navigator.mediaDevices.getDisplayMedia).toHaveBeenCalled());

        rerender({ config: defaultConfig, isCaptionMode: false });
        expect(captionWindowService.close).toHaveBeenCalled();

        await act(async () => {
            resolveDisplayMedia(mockStream);
        });

        await new Promise((resolve) => setTimeout(resolve, 50));

        expect(captionWindowService.open).not.toHaveBeenCalled();
        expect(transcriptionServiceMocks.recordStart).not.toHaveBeenCalled();
        expect(transcriptionServiceMocks.recordStop).not.toHaveBeenCalled();
    });
});
