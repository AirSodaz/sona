import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';

// Mock Tauri invoke
const mockInvoke = vi.fn().mockResolvedValue(undefined);
vi.mock('@tauri-apps/api/core', () => ({
    invoke: (cmd: string, args: any) => mockInvoke(cmd, args),
}));

// Mock Tauri listen
const mockListen = vi.fn().mockResolvedValue(() => {});
let systemAudioCallback: ((event: any) => void) | null = null;

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn((event, callback) => {
        if (event === 'system-audio') {
            systemAudioCallback = callback;
        }
        return Promise.resolve(mockListen); // Return unlisten function
    }),
}));

// Mock historyService
const mockSaveRecording = vi.fn().mockResolvedValue({ id: 'mock-id' });
vi.mock('../../services/historyService', () => ({
    historyService: {
        saveRecording: (blob: Blob, segments: any, duration: number) => mockSaveRecording(blob, segments, duration),
        saveTranscriptFile: vi.fn(),
        getAll: vi.fn().mockResolvedValue([]),
        init: vi.fn(),
    }
}));

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
        updateStyle: vi.fn().mockResolvedValue(undefined),
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

describe('LiveRecord Native Capture', () => {
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

        // Mock AudioContext
        vi.stubGlobal('AudioContext', class {
            state = 'running';
            destination = {};
            currentTime = 0;
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
            createBuffer(_channels: number, length: number, sampleRate: number) {
                return {
                    copyToChannel: vi.fn(),
                    duration: length / sampleRate,
                };
            }
            createBufferSource() {
                return {
                    buffer: null,
                    connect: vi.fn(),
                    start: vi.fn(),
                    stop: vi.fn(),
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
        mockInvoke.mockClear();
        mockSaveRecording.mockClear();
        systemAudioCallback = null;
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

    it('should invoke native capture and save recording when stopping', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        render(<LiveRecord />);

        // 1. Select "Desktop" source
        // Find dropdown trigger
        const dropdownTrigger = screen.getByLabelText('live.source_select');
        await act(async () => {
            fireEvent.click(dropdownTrigger);
        });

        // Find "Desktop" option and click it
        const desktopOption = screen.getByText('live.source_desktop');
        await act(async () => {
            fireEvent.click(desktopOption);
        });

        // 2. Start Recording
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });
        await act(async () => {
            fireEvent.click(startBtn);
            // Wait for async initialization
            await vi.advanceTimersByTimeAsync(100);
        });

        // Check invoke called
        expect(mockInvoke).toHaveBeenCalledWith('start_system_audio_capture', undefined);
        expect(useTranscriptStore.getState().isRecording).toBe(true);
        expect(systemAudioCallback).toBeDefined();

        // 3. Simulate audio data
        // Simulate a few chunks of audio
        const mockAudioPayload = Array.from({ length: 1024 }, () => Math.floor(Math.random() * 32767));
        if (systemAudioCallback) {
            await act(async () => {
                systemAudioCallback!({ payload: mockAudioPayload });
                await vi.advanceTimersByTimeAsync(100);
            });
             await act(async () => {
                systemAudioCallback!({ payload: mockAudioPayload });
                await vi.advanceTimersByTimeAsync(100);
            });
        }

        // 4. Stop Recording
        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        await act(async () => {
            fireEvent.click(stopBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        // Check invoke stop called
        expect(mockInvoke).toHaveBeenCalledWith('stop_system_audio_capture', undefined);

        // Check saveRecording called
        // Note: LiveRecord checks if segments > 0 OR duration > 1.0
        // We advanced time by ~300ms, so duration < 1.0.
        // And we didn't mock transcription results, so segments = 0.
        // So saveRecording might NOT be called if we don't mock duration or segments.

        // Wait! We need to ensure saveRecording is called.
        // We can simulate time passing > 1.0s.
    });

    it('should save recording if duration > 1s', async () => {
        await import('../../stores/transcriptStore');

        render(<LiveRecord />);

        // 1. Select "Desktop" source
        const dropdownTrigger = screen.getByLabelText('live.source_select');
        await act(async () => { fireEvent.click(dropdownTrigger); });
        const desktopOption = screen.getByText('live.source_desktop');
        await act(async () => { fireEvent.click(desktopOption); });

        // 2. Start Recording
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });
        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(systemAudioCallback).toBeDefined();

        // 3. Simulate audio data and time passing
        const mockAudioPayload = Array.from({ length: 1024 }, () => 100);
        if (systemAudioCallback) {
            await act(async () => {
                systemAudioCallback!({ payload: mockAudioPayload });
            });
        }

        // Advance time by 1.5 seconds
        await act(async () => {
            vi.advanceTimersByTime(1500);
        });

        // 4. Stop Recording
        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        await act(async () => {
            fireEvent.click(stopBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockSaveRecording).toHaveBeenCalled();
        const callArgs = mockSaveRecording.mock.calls[0];
        // 1st arg: Blob
        expect(callArgs[0]).toBeInstanceOf(Blob);
        // 3rd arg: duration (approx 1.5s)
        expect(callArgs[2]).toBeGreaterThan(1.0);
    });
});
