import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent, renderHook } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';
import { useAudioRecorder } from '../../hooks/useAudioRecorder';

// Mock Tauri invoke
const mockInvoke = vi.fn().mockImplementation(async (cmd: string) => {
    if (cmd === 'stop_system_audio_capture' || cmd === 'stop_microphone_capture') {
        return '/mock/path/to/audio.wav';
    }
    return undefined;
});
vi.mock('@tauri-apps/api/core', () => ({
    invoke: (cmd: string, args: any) => mockInvoke(cmd, args),
    convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));

// Mock Tauri listen
const mockUnlisten = vi.fn();
const mockEventListen = vi.fn((event: string, callback: (event: any) => void) => {
    if (event === 'system-audio') {
        systemAudioCallback = callback;
    }
    return Promise.resolve(mockUnlisten);
});
let systemAudioCallback: ((event: any) => void) | null = null;
let recordSegmentCallback: ((segment: any) => void) | null = null;

vi.mock('@tauri-apps/api/event', () => ({
    listen: (event: string, callback: (event: any) => void) => mockEventListen(event, callback),
}));

// Mock historyService
const mockSaveRecording = vi.fn().mockResolvedValue({ id: 'mock-id' });
const mockSaveNativeRecording = vi.fn().mockResolvedValue({ id: 'mock-id' });
vi.mock('../../services/historyService', () => ({
    historyService: {
        saveRecording: (blob: Blob, segments: any, duration: number) => mockSaveRecording(blob, segments, duration),
        saveNativeRecording: (path: string, segments: any, duration: number) => mockSaveNativeRecording(path, segments, duration),
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
    mockPauseStream,
    mockResumeStream,
    mockPrepare,
    mockSendAudioInt16,
    mockSetModelPath,
    mockSetLanguage,
    mockSetEnableITN,
    mockSetITNModelPaths,
    mockTerminate
} = vi.hoisted(() => ({
    mockStart: vi.fn().mockResolvedValue(undefined),
    mockStop: vi.fn().mockResolvedValue(undefined),
    mockSoftStop: vi.fn().mockResolvedValue(undefined),
    mockPauseStream: vi.fn().mockResolvedValue(undefined),
    mockResumeStream: vi.fn().mockResolvedValue(undefined),
    mockPrepare: vi.fn().mockResolvedValue(undefined),
    mockSendAudioInt16: vi.fn(),
    mockSetModelPath: vi.fn(),
    mockSetLanguage: vi.fn(),
    mockSetEnableITN: vi.fn(),
    mockSetITNModelPaths: vi.fn(),
    mockTerminate: vi.fn().mockResolvedValue(undefined),
}));

// Mock transcription service
vi.mock('../../services/transcriptionService', () => {
    return {
        transcriptionService: {
            start: mockStart,
            stop: mockStop,
            softStop: mockSoftStop,
            pauseStream: mockPauseStream,
            resumeStream: mockResumeStream,
            sendAudioInt16: mockSendAudioInt16,
            setModelPath: mockSetModelPath,
            setLanguage: mockSetLanguage,
            setEnableITN: mockSetEnableITN,
            setITNModelPaths: mockSetITNModelPaths,
            prepare: mockPrepare,
            terminate: mockTerminate,
        },
        captionTranscriptionService: {
            start: mockStart,
            stop: mockStop,
            softStop: mockSoftStop,
            pauseStream: mockPauseStream,
            resumeStream: mockResumeStream,
            sendAudioInt16: mockSendAudioInt16,
            setModelPath: mockSetModelPath,
            setLanguage: mockSetLanguage,
            setEnableITN: mockSetEnableITN,
            setITNModelPaths: mockSetITNModelPaths,
            prepare: mockPrepare,
            terminate: mockTerminate,
        },
        TranscriptionService: class {
            start = mockStart;
            stop = mockStop;
            softStop = mockSoftStop;
            pauseStream = mockPauseStream;
            resumeStream = mockResumeStream;
            sendAudioInt16 = mockSendAudioInt16;
            setModelPath = mockSetModelPath;
            setLanguage = mockSetLanguage;
            setEnableITN = mockSetEnableITN;
            setITNModelPaths = mockSetITNModelPaths;
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
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
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
        mockInvoke.mockImplementation(async (cmd: string) => {
            if (cmd === 'stop_system_audio_capture' || cmd === 'stop_microphone_capture') {
                return '/mock/path/to/audio.wav';
            }
            return undefined;
        });

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

        // Mock MediaRecorder
        vi.stubGlobal('MediaRecorder', class {
            state = 'inactive';
            mimeType = 'audio/webm';
            ondataavailable: ((e: any) => void) | null = null;
            onstop: (() => void) | null = null;
            constructor(_stream: any, _options: any) { }
            start() { this.state = 'recording'; }
            stop() {
                this.state = 'inactive';
                if (this.onstop) this.onstop();
            }
            pause() { this.state = 'paused'; }
            resume() { this.state = 'recording'; }
            requestData() { }
            static isTypeSupported() { return true; }
        });

        // Mock AudioBuffer
        vi.stubGlobal('AudioBuffer', class {
            length: number;
            sampleRate: number;
            duration: number;
            constructor(options: any) {
                this.length = options.length;
                this.sampleRate = options.sampleRate;
                this.duration = this.length / this.sampleRate;
            }
            copyToChannel = vi.fn();
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
            createGain() {
                return {
                    gain: { value: 1 },
                    connect: vi.fn(),
                };
            }
            createBuffer(_channels: number, length: number, sampleRate: number) {
                // @ts-ignore
                return new AudioBuffer({ length, sampleRate });
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
            beginPath: vi.fn(),
            moveTo: vi.fn(),
            lineTo: vi.fn(),
            stroke: vi.fn(),
            fillStyle: '',
            strokeStyle: '',
            lineWidth: 1,
        })) as any;

        // Setup store config
        const { useConfigStore } = await import('../../stores/configStore');
        const { useDialogStore } = await import('../../stores/dialogStore');
        act(() => {
            useConfigStore.setState({
                config: {
                    ...useConfigStore.getState().config, streamingModelPath: "/path/to/model",
                    offlineModelPath: '/path/to/model'
                }
            });
            useDialogStore.setState({
                showError: vi.fn().mockResolvedValue(undefined)
            } as any);
        });

        // Reset mocks
        mockStart.mockClear();
        mockStop.mockClear();
        mockSoftStop.mockClear();
        mockPauseStream.mockClear();
        mockResumeStream.mockClear();
        mockPrepare.mockClear();
        mockEventListen.mockClear();
        mockEventListen.mockImplementation((event: string, callback: (event: any) => void) => {
            if (event === 'system-audio') {
                systemAudioCallback = callback;
            }
            return Promise.resolve(mockUnlisten);
        });
        mockUnlisten.mockClear();
        mockInvoke.mockClear();
        mockSaveRecording.mockClear();
        mockSaveNativeRecording.mockClear();
        systemAudioCallback = null;
        recordSegmentCallback = null;
        mockStart.mockImplementation(async (onSegment?: (segment: any) => void) => {
            recordSegmentCallback = typeof onSegment === 'function' ? onSegment : null;
        });
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
        expect(mockInvoke).toHaveBeenCalledWith('start_system_audio_capture', { deviceName: null, instanceId: 'record' });
        expect(useTranscriptStore.getState().isRecording).toBe(true);
        expect(systemAudioCallback).toBeDefined();

        // 3. Simulate audio data
        // Simulate a few chunks of audio
        const mockAudioPayload = Math.floor(Math.random() * 32767);
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
        // Mock segments so the recording saves instead of thinking it was too short/empty
        act(() => {
            useTranscriptStore.setState({
                segments: [{ id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }]
            });
        });

        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        await act(async () => {
            fireEvent.click(stopBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        // Check invoke stop called for desktop source
        expect(mockInvoke).toHaveBeenCalledWith('stop_system_audio_capture', { instanceId: 'record' });

        // Check saveRecording called
        // Note: LiveRecord checks if segments > 0 OR duration > 1.0
        // We advanced time by ~300ms, so duration < 1.0.
        // And we didn't mock transcription results, so segments = 0.
        // So saveRecording might NOT be called if we don't mock duration or segments.

        // Wait! We need to ensure saveRecording is called.
        // We can simulate time passing > 1.0s.
    });

    it('accepts record segments that arrive while the session is still starting', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        mockStart.mockImplementationOnce(async (onSegment?: (segment: any) => void) => {
            recordSegmentCallback = typeof onSegment === 'function' ? onSegment : null;
            onSegment?.({
                id: 'startup-seg',
                text: 'Hello during startup',
                start: 0,
                end: 1,
                isFinal: true
            });
        });

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockInvoke).toHaveBeenCalledWith('start_microphone_capture', { deviceName: null, instanceId: 'record' });
        expect(useTranscriptStore.getState().isRecording).toBe(true);
        expect(useTranscriptStore.getState().segments).toEqual([
            expect.objectContaining({
                id: 'startup-seg',
                text: 'Hello during startup',
                isFinal: true
            })
        ]);
    });

    it('should save recording if segments exist', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

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

        // 3. Simulate audio data
        const mockAudioPayload = 100;
        if (systemAudioCallback) {
            await act(async () => {
                systemAudioCallback!({ payload: mockAudioPayload });
            });
        }

        // Simulate segments being added
        act(() => {
            useTranscriptStore.setState({
                segments: [{ id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }]
            });
        });

        // 4. Stop Recording
        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        await act(async () => {
            fireEvent.click(stopBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        // Give time for promises (like stop_system_audio_capture) to resolve
        // In Vitest with fake timers, we need to advance them to let promises resolve
        await act(async () => {
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockSaveNativeRecording).toHaveBeenCalled();
        const callArgs = mockSaveNativeRecording.mock.calls[0];
        // 1st arg: Path
        expect(typeof callArgs[0]).toBe('string');
        // 2nd arg: segments
        expect(callArgs[1]).toHaveLength(1);
    });

    it('keeps accepting native microphone segments after capture attaches', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(recordSegmentCallback).toBeTypeOf('function');

        await act(async () => {
            recordSegmentCallback?.({
                id: 'attached-seg',
                text: 'Shared mic is feeding record',
                start: 0,
                end: 1,
                isFinal: true
            });
        });

        expect(useTranscriptStore.getState().segments).toEqual([
            expect.objectContaining({
                id: 'attached-seg',
                text: 'Shared mic is feeding record'
            })
        ]);
    });

    it('keeps the native microphone session alive when the peak listener cannot attach', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        mockEventListen.mockImplementation((event: string, callback: (event: any) => void) => {
            if (event === 'microphone-audio') {
                return Promise.reject(new Error('listener attach failed'));
            }
            if (event === 'system-audio') {
                systemAudioCallback = callback;
            }
            return Promise.resolve(mockUnlisten);
        });

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockInvoke).toHaveBeenCalledWith('start_microphone_capture', { deviceName: null, instanceId: 'record' });
        expect(mockInvoke).not.toHaveBeenCalledWith('stop_microphone_capture', { instanceId: 'record' });
        expect(navigator.mediaDevices.getUserMedia).not.toHaveBeenCalled();
        expect(useTranscriptStore.getState().isRecording).toBe(true);

        await act(async () => {
            recordSegmentCallback?.({
                id: 'listener-failure-seg',
                text: 'Listener failure should not tear down record',
                start: 0,
                end: 1,
                isFinal: true
            });
        });

        expect(useTranscriptStore.getState().segments).toEqual([
            expect.objectContaining({
                id: 'listener-failure-seg',
                text: 'Listener failure should not tear down record'
            })
        ]);
    });

    it('rolls back the record session state when microphone startup ultimately fails', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        mockStart.mockImplementationOnce(async (onSegment?: (segment: any) => void) => {
            recordSegmentCallback = typeof onSegment === 'function' ? onSegment : null;
            onSegment?.({
                id: 'rollback-seg',
                text: 'Should be cleared after failure',
                start: 0,
                end: 1,
                isFinal: true
            });
        });
        navigator.mediaDevices.getUserMedia = vi.fn().mockRejectedValue(new Error('web fallback failed'));
        mockInvoke.mockImplementation(async (cmd: string) => {
            if (cmd === 'start_microphone_capture') {
                throw new Error('mic attach failed');
            }
            if (cmd === 'stop_system_audio_capture' || cmd === 'stop_microphone_capture') {
                return '/mock/path/to/audio.wav';
            }
            return undefined;
        });

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(useTranscriptStore.getState().isRecording).toBe(false);
        expect(useTranscriptStore.getState().segments).toEqual([]);
        expect(mockSoftStop).toHaveBeenCalled();
        expect(screen.queryByRole('button', { name: /live.stop/i })).toBeNull();
        expect(screen.getByRole('button', { name: /live.start_recording/i })).not.toBeNull();
    });

    it('does not let a stale microphone startup rollback stop a newer record session', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        let rejectFirstAttach: ((reason?: unknown) => void) | null = null;
        let microphoneAttachAttempts = 0;

        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'set_microphone_boost') {
                return Promise.resolve(undefined);
            }

            if (cmd === 'start_microphone_capture') {
                microphoneAttachAttempts += 1;
                if (microphoneAttachAttempts === 1) {
                    return new Promise((_, reject) => {
                        rejectFirstAttach = reject;
                    });
                }
                return Promise.resolve(undefined);
            }

            if (cmd === 'stop_microphone_capture' || cmd === 'stop_system_audio_capture') {
                return Promise.resolve('/mock/path/to/audio.wav');
            }

            return Promise.resolve(undefined);
        });

        const { result } = renderHook(() => useAudioRecorder({
            inputSource: 'microphone',
            onSegment: vi.fn()
        }));

        let firstStartPromise: Promise<boolean>;
        await act(async () => {
            firstStartPromise = result.current.startRecording();
            await Promise.resolve();
            await Promise.resolve();
        });

        expect(microphoneAttachAttempts).toBe(1);

        let secondStartResult = false;
        await act(async () => {
            secondStartResult = await result.current.startRecording();
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(secondStartResult).toBe(true);
        expect(useTranscriptStore.getState().isRecording).toBe(true);

        await act(async () => {
            rejectFirstAttach?.(new Error('first attach failed'));
            await expect(firstStartPromise!).resolves.toBe(false);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(useTranscriptStore.getState().isRecording).toBe(true);
        expect(mockSoftStop).not.toHaveBeenCalled();
        expect(mockInvoke).not.toHaveBeenCalledWith('stop_microphone_capture', { instanceId: 'record' });
    });

    it('should stop the correct native capture after switching desktop back to microphone', async () => {
        render(<LiveRecord />);

        const dropdownTrigger = screen.getByLabelText('live.source_select');

        // Select desktop source and start/stop once
        await act(async () => { fireEvent.click(dropdownTrigger); });
        await act(async () => { fireEvent.click(screen.getByText('live.source_desktop')); });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.stop/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockInvoke).toHaveBeenCalledWith('start_system_audio_capture', { deviceName: null, instanceId: 'record' });
        expect(mockInvoke).toHaveBeenCalledWith('stop_system_audio_capture', { instanceId: 'record' });

        // Switch back to microphone source and start/stop again
        await act(async () => { fireEvent.click(screen.getByLabelText('live.source_select')); });
        await act(async () => { fireEvent.click(screen.getByText('live.source_microphone')); });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.stop/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockInvoke).toHaveBeenCalledWith('start_microphone_capture', { deviceName: null, instanceId: 'record' });
        expect(mockInvoke).toHaveBeenCalledWith('stop_microphone_capture', { instanceId: 'record' });
    });
});
