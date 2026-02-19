import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';

// Hoist mocks to share between singleton and class instances
const {
    mockStart,
    mockStop,
    mockSoftStop,
    mockPrepare,
    mockSendAudioInt16,
    mockSetModelPath,
    mockSetLanguage,
    mockSetEnableITN,
    mockSetITNModelPaths,
    mockSetPunctuationModelPath,
    mockSetCtcModelPath,
    mockSetVadModelPath,
    mockSetVadBufferSize,
    mockTerminate
} = vi.hoisted(() => ({
    mockStart: vi.fn().mockResolvedValue(undefined),
    mockStop: vi.fn().mockResolvedValue(undefined),
    mockSoftStop: vi.fn().mockResolvedValue(undefined),
    mockPrepare: vi.fn().mockResolvedValue(undefined),
    mockSendAudioInt16: vi.fn(),
    mockSetModelPath: vi.fn(),
    mockSetLanguage: vi.fn(),
    mockSetEnableITN: vi.fn(),
    mockSetITNModelPaths: vi.fn(),
    mockSetPunctuationModelPath: vi.fn(),
    mockSetCtcModelPath: vi.fn(),
    mockSetVadModelPath: vi.fn(),
    mockSetVadBufferSize: vi.fn(),
    mockTerminate: vi.fn().mockResolvedValue(undefined),
}));

// Mock transcription service
vi.mock('../../services/transcriptionService', () => {
    return {
        transcriptionService: {
            start: mockStart,
            stop: mockStop,
            softStop: mockSoftStop,
            sendAudioInt16: mockSendAudioInt16,
            setModelPath: mockSetModelPath,
            setLanguage: mockSetLanguage,
            setEnableITN: mockSetEnableITN,
            setITNModelPaths: mockSetITNModelPaths,
            setPunctuationModelPath: mockSetPunctuationModelPath,
            setCtcModelPath: mockSetCtcModelPath,
            setVadModelPath: mockSetVadModelPath,
            setVadBufferSize: mockSetVadBufferSize,
            prepare: mockPrepare,
            terminate: mockTerminate,
        },
        TranscriptionService: class {
            start = mockStart;
            stop = mockStop;
            softStop = mockSoftStop;
            sendAudioInt16 = mockSendAudioInt16;
            setModelPath = mockSetModelPath;
            setLanguage = mockSetLanguage;
            setEnableITN = mockSetEnableITN;
            setITNModelPaths = mockSetITNModelPaths;
            setPunctuationModelPath = mockSetPunctuationModelPath;
            setCtcModelPath = mockSetCtcModelPath;
            setVadModelPath = mockSetVadModelPath;
            setVadBufferSize = mockSetVadBufferSize;
            prepare = mockPrepare;
            terminate = mockTerminate;
        }
    };
});

// Mock model service
vi.mock('../../services/modelService', () => ({
    modelService: {
        isITNModelInstalled: vi.fn().mockResolvedValue(false),
        getITNModelPath: vi.fn().mockResolvedValue('/path/to/itn'),
        getEnabledITNModelPaths: vi.fn().mockResolvedValue(['/path/to/itn']),
    }
}));

// Mock caption window service
vi.mock('../../services/captionWindowService', () => ({
    captionWindowService: {
        open: vi.fn().mockResolvedValue(undefined),
        close: vi.fn().mockResolvedValue(undefined),
        isOpen: vi.fn().mockResolvedValue(true),
        sendSegments: vi.fn().mockResolvedValue(undefined),
        setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
        setClickThrough: vi.fn().mockResolvedValue(undefined),
    }
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: vi.fn(() => Promise.resolve(false)),
    remove: vi.fn(() => Promise.resolve()),
    mkdir: vi.fn(() => Promise.resolve()),
    writeTextFile: vi.fn(() => Promise.resolve()),
    readTextFile: vi.fn(() => Promise.resolve('')),
    BaseDirectory: { AppData: 1, Resource: 2, AppLocalData: 3 },
}));

// Mock translation
const mockT = (key: string) => key;
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: mockT,
    }),
}));

// Mock icons
vi.mock('lucide-react', () => ({
    Pause: () => 'Pause',
    Play: () => 'Play',
    Square: () => 'Square',
    Mic: () => 'Mic',
    Monitor: () => 'Monitor',
    FileAudio: () => 'FileAudio',
}));

describe('LiveRecord', () => {
    beforeEach(async () => {
        vi.useFakeTimers();

        const raf = vi.fn((cb) => setTimeout(cb, 16));
        const caf = vi.fn((id) => clearTimeout(id));

        vi.stubGlobal('requestAnimationFrame', raf);
        vi.stubGlobal('cancelAnimationFrame', caf);
        window.requestAnimationFrame = raf as any;
        window.cancelAnimationFrame = caf as any;

        // Mock Media APIs
        vi.stubGlobal('MediaStream', class {
            tracks: any[];
            constructor(tracks?: any[]) {
                this.tracks = tracks || [{ stop: vi.fn() }];
            }
            getAudioTracks() { return this.tracks; }
            getVideoTracks() { return []; }
            getTracks() { return this.tracks; }
        });

        vi.stubGlobal('MediaRecorder', class {
            state = 'inactive';
            stream: any;
            ondataavailable: ((e: any) => void) | null = null;
            onstop: (() => void) | null = null;
            mimeType = 'audio/webm';

            constructor(stream: any) {
                this.stream = stream;
            }
            start() { this.state = 'recording'; }
            stop() {
                this.state = 'inactive';
                if (this.onstop) this.onstop();
            }
            pause() { this.state = 'paused'; }
            resume() { this.state = 'recording'; }
            static isTypeSupported() { return true; }
        });

        // Mock AudioContext
        vi.stubGlobal('AudioContext', class {
            state = 'running';
            destination = {};
            createMediaStreamSource() {
                return { connect: vi.fn() };
            }
            createAnalyser() {
                return {
                    fftSize: 2048,
                    frequencyBinCount: 1024,
                    getByteFrequencyData: vi.fn(),
                    connect: vi.fn(),
                };
            }
            audioWorklet = {
                addModule: vi.fn().mockResolvedValue(undefined),
            };
            close() { return Promise.resolve(); }
            suspend() { return Promise.resolve(); }
            resume() { return Promise.resolve(); }
        });

        vi.stubGlobal('AudioWorkletNode', class {
            port = {
                onmessage: null,
            };
            connect = vi.fn();
        });

        // Mock getUserMedia
        if (!navigator.mediaDevices) {
            Object.defineProperty(navigator, 'mediaDevices', {
                value: {},
                writable: true,
            });
        }
        navigator.mediaDevices.getUserMedia = vi.fn().mockResolvedValue(new MediaStream());
        navigator.mediaDevices.getDisplayMedia = vi.fn().mockResolvedValue(new MediaStream());

        // Mock Canvas
        HTMLCanvasElement.prototype.getContext = vi.fn(() => ({
            clearRect: vi.fn(),
            createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
            fillRect: vi.fn(),
            fillStyle: '',
        })) as any;

        // Setup store config
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        act(() => {
            useTranscriptStore.setState({
                config: { ...useTranscriptStore.getState().config, offlineModelPath: '/path/to/model' }
            });
        });

        // Reset mocks
        mockStart.mockClear();
        mockStop.mockClear();
        mockSoftStop.mockClear();
        mockPrepare.mockClear();
    });

    afterEach(async () => {
        // Reset store state
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        act(() => {
            useTranscriptStore.setState({
                isRecording: false,
                isPaused: false,
                isCaptionMode: false,
                segments: [],
                audioUrl: null,
            });
        });
        vi.useRealTimers();
        vi.clearAllMocks();
    });

    it('should start recording when Start button is clicked', async () => {
        render(<LiveRecord />);
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(screen.getByRole('button', { name: /live.stop/i })).toBeTruthy();
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        expect(useTranscriptStore.getState().isRecording).toBe(true);
    });

    it('should start caption mode independently without recording', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        const { captionWindowService } = await import('../../services/captionWindowService');
        // We use mockStart which intercepts calls to any TranscriptionService instance (singleton or new)

        render(<LiveRecord />);
        const captionSwitch = screen.getByRole('switch', { name: /live.caption_mode/i });

        await act(async () => {
            fireEvent.click(captionSwitch);
            await vi.advanceTimersByTimeAsync(100);
        });

        // Check stores
        expect(useTranscriptStore.getState().isCaptionMode).toBe(true);
        expect(useTranscriptStore.getState().isRecording).toBe(false);

        // Check services
        expect(captionWindowService.open).toHaveBeenCalled();
        expect(mockStart).toHaveBeenCalled();
    });

    it('should allow recording while caption mode is active', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        render(<LiveRecord />);

        // 1. Start Caption
        const captionSwitch = screen.getByRole('switch', { name: /live.caption_mode/i });
        await act(async () => {
            fireEvent.click(captionSwitch);
            await vi.advanceTimersByTimeAsync(100);
        });
        expect(useTranscriptStore.getState().isCaptionMode).toBe(true);

        // 2. Start Recording
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });
        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(useTranscriptStore.getState().isRecording).toBe(true);
        expect(useTranscriptStore.getState().isCaptionMode).toBe(true);
    });

    it('should NOT stop recording when caption mode is toggled OFF', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        const { captionWindowService } = await import('../../services/captionWindowService');

        render(<LiveRecord />);

        // 1. Start Caption
        const captionSwitch = screen.getByRole('switch', { name: /live.caption_mode/i });
        await act(async () => {
            fireEvent.click(captionSwitch);
            await vi.advanceTimersByTimeAsync(100);
        });

        // 2. Start Recording
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });
        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        // 3. Stop Caption
        await act(async () => {
            fireEvent.click(captionSwitch);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(useTranscriptStore.getState().isCaptionMode).toBe(false);
        expect(useTranscriptStore.getState().isRecording).toBe(true); // Should still be true
        expect(captionWindowService.close).toHaveBeenCalled();
    });

    it('should toggle recording with Ctrl+Space', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        render(<LiveRecord />);

        // Start recording with Ctrl+Space
        await act(async () => {
            fireEvent.keyDown(window, { key: ' ', code: 'Space', ctrlKey: true });
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(screen.getByRole('button', { name: /live.stop/i })).toBeTruthy();
        expect(useTranscriptStore.getState().isRecording).toBe(true);

        // Stop recording with Ctrl+Space
        await act(async () => {
            fireEvent.keyDown(window, { key: ' ', code: 'Space', ctrlKey: true });
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(screen.getByRole('button', { name: /live.start_recording/i })).toBeTruthy();
        expect(useTranscriptStore.getState().isRecording).toBe(false);
    });
});
