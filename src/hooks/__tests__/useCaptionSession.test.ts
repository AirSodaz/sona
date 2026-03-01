import { renderHook, act, waitFor } from '@testing-library/react';
import { useCaptionSession } from '../useCaptionSession';
import { captionWindowService } from '../../services/captionWindowService';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { AppConfig } from '../../types/transcript';

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockRejectedValue(new Error('Native capture not supported')),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

vi.mock('../../services/captionWindowService', () => ({
    captionWindowService: {
        open: vi.fn(),
        close: vi.fn(),
        sendSegments: vi.fn(),
        updateStyle: vi.fn().mockResolvedValue(undefined),
    }
}));

vi.mock('../../services/modelService', () => ({
    modelService: {
        getEnabledITNModelPaths: vi.fn().mockResolvedValue([]),
    }
}));

// Mock global transcriptionService instance
const transcriptionMocks = vi.hoisted(() => ({
    start: vi.fn(),
    stop: vi.fn(),
    sendAudioInt16: vi.fn()
}));

vi.mock('../../services/transcriptionService', () => {
    return {
        transcriptionService: {
            start: transcriptionMocks.start,
            stop: transcriptionMocks.stop,
            sendAudioInt16: transcriptionMocks.sendAudioInt16
        },
        captionTranscriptionService: {
            start: transcriptionMocks.start,
            stop: transcriptionMocks.stop,
            sendAudioInt16: transcriptionMocks.sendAudioInt16
        }
    };
});

// Mock AudioContext
const audioContextMocks = vi.hoisted(() => ({
    close: vi.fn(),
    resume: vi.fn(),
    addModule: vi.fn(),
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

    beforeEach(() => {
        vi.clearAllMocks();

        // Setup MediaStream mock
        mockStream = {
            getAudioTracks: () => [{ onended: null, stop: vi.fn() }],
            getVideoTracks: () => [{ stop: vi.fn() }],
            getTracks: () => [{ stop: vi.fn() }],
        };

        if (!navigator.mediaDevices) {
            Object.defineProperty(navigator, 'mediaDevices', {
                value: {},
                writable: true,
            });
        }
    });

    const defaultConfig: AppConfig = {
        offlineModelPath: '/path/to/model',
        language: 'en',
    } as AppConfig;

    it('should NOT open caption window if toggled off during initialization (Race Condition)', async () => {
        let resolveDisplayMedia: (value: any) => void = () => { };

        const mockGetDisplayMedia = vi.fn().mockImplementation(() => {
            return new Promise((resolve) => {
                resolveDisplayMedia = resolve;
            });
        });
        navigator.mediaDevices.getDisplayMedia = mockGetDisplayMedia;

        // 1. Render with caption mode ON
        const { rerender } = renderHook(
            (props) => useCaptionSession(props.config, props.isCaptionMode),
            { initialProps: { config: defaultConfig, isCaptionMode: true } }
        );

        // Expect getDisplayMedia to be called
        await waitFor(() => expect(mockGetDisplayMedia).toHaveBeenCalled());

        // 2. Toggle OFF immediately (while awaiting getDisplayMedia)
        rerender({ config: defaultConfig, isCaptionMode: false });

        // Expect close to be called immediately
        expect(captionWindowService.close).toHaveBeenCalled();

        // 3. Resolve getDisplayMedia to simulate completion of async task
        await act(async () => {
            resolveDisplayMedia(mockStream);

            // Also resolve addModule and service.start
            audioContextMocks.addModule.mockResolvedValue(undefined);
            transcriptionMocks.start.mockImplementation((_onSegment: any, _onError: any) => Promise.resolve());
        });

        // Wait a bit for async operations to proceed
        await new Promise(r => setTimeout(r, 50));

        // 4. Assert that open() was NOT called
        // If the bug exists, this will fail
        expect(captionWindowService.open).not.toHaveBeenCalled();
    });
});
