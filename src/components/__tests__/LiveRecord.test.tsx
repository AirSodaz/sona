import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';

// Mock transcription service
vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        start: vi.fn().mockResolvedValue(undefined),
        stop: vi.fn().mockResolvedValue(undefined),
        softStop: vi.fn().mockResolvedValue(undefined),
        sendAudioInt16: vi.fn(),
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),
        setPunctuationModelPath: vi.fn(),
        setCtcModelPath: vi.fn(),
        setVadModelPath: vi.fn(),
        setVadBufferSize: vi.fn(),
    }
}));

// Mock model service
vi.mock('../../services/modelService', () => ({
    modelService: {
        isITNModelInstalled: vi.fn().mockResolvedValue(false),
        getITNModelPath: vi.fn().mockResolvedValue('/path/to/itn'),
        getEnabledITNModelPaths: vi.fn().mockResolvedValue(['/path/to/itn']),
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
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
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
        vi.stubGlobal('MediaRecorder', class {
            state = 'inactive';
            stream: MediaStream;
            ondataavailable: ((e: any) => void) | null = null;
            onstop: (() => void) | null = null;
            mimeType = 'audio/webm';

            constructor(stream: MediaStream) {
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
        navigator.mediaDevices.getUserMedia = vi.fn().mockResolvedValue({
            getTracks: () => [{ stop: vi.fn() }],
        });

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
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
    });

    it('should increment timer correctly', async () => {
        render(<LiveRecord />);

        // Find the start button (it's the only button initially)
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        // Wait for recording to start (Stop button appears)
        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        expect(stopBtn).toBeTruthy();

        // Advance 3 seconds
        await act(async () => {
            vi.advanceTimersByTime(3000);
        });

        // Check text. formatTime(3) -> "00:03"
        const timeDisplay = screen.getByText(/00:03/);
        expect(timeDisplay).toBeTruthy();
    }, 10000);

    it('should reset player state when recording starts', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        // Set up initial state with an audio file
        act(() => {
            useTranscriptStore.setState({ audioUrl: 'blob:test', isPlaying: true });
        });

        expect(useTranscriptStore.getState().audioUrl).toBe('blob:test');

        render(<LiveRecord />);
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        // Wait for recording to start
        expect(screen.getByRole('button', { name: /live.stop/i })).toBeTruthy();

        // Verify audioUrl is reset to null
        expect(useTranscriptStore.getState().audioUrl).toBeNull();
        expect(useTranscriptStore.getState().isPlaying).toBe(false); // setAudioFile(null) also sets isPlaying to false
    });

    it('should finalize the last segment when recording stops', async () => {
        render(<LiveRecord />);
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        const { transcriptionService } = await import('../../services/transcriptionService');

        // Capture the onSegment callback passed to transcriptionService.start
        let onSegmentCallback: ((segment: any) => void) | undefined;
        (transcriptionService.start as any).mockImplementation((onSegment: any) => {
            onSegmentCallback = onSegment;
            return Promise.resolve();
        });

        // Mock softStop to simulate sidecar finalizing the segment
        (transcriptionService.softStop as any).mockImplementation(async () => {
            if (onSegmentCallback) {
                onSegmentCallback({
                    id: 'seg-1',
                    text: 'Incomplete sentence.',
                    start: 0,
                    end: 1,
                    isFinal: true,
                });
            }
        });

        // Start recording
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });
        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        // Wait for recording to start
        expect(screen.getByRole('button', { name: /live.stop/i })).toBeTruthy();

        // Simulate receiving a partial segment
        act(() => {
            if (onSegmentCallback) {
                onSegmentCallback({
                    id: 'seg-1',
                    text: 'Incomplete sentence',
                    start: 0,
                    end: 1,
                    isFinal: false,
                });
            }
        });

        // Verify segment is in store and is not final
        expect(useTranscriptStore.getState().segments).toHaveLength(1);
        expect(useTranscriptStore.getState().segments[0].isFinal).toBe(false);

        // Stop recording
        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        await act(async () => {
            fireEvent.click(stopBtn);
        });

        // Verify softStop was called
        expect(transcriptionService.softStop).toHaveBeenCalled();

        // Verify segment is now final (updated by the mock)
        expect(useTranscriptStore.getState().segments).toHaveLength(1);
        expect(useTranscriptStore.getState().segments[0].isFinal).toBe(true);
        expect(useTranscriptStore.getState().segments[0].text).toBe('Incomplete sentence.');
    });

    it('should show alert when microphone permission is denied', async () => {
        // Mock NotAllowedError
        navigator.mediaDevices.getUserMedia = vi.fn().mockRejectedValue({
            name: 'NotAllowedError',
            message: 'Permission denied',
        });

        const { useDialogStore } = await import('../../stores/dialogStore');
        const alertSpy = vi.spyOn(useDialogStore.getState(), 'alert');

        render(<LiveRecord />);

        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
        });

        expect(alertSpy).toHaveBeenCalledWith(
            expect.stringContaining('live.mic_error'),
            expect.objectContaining({ variant: 'error' })
        );
        expect(alertSpy).toHaveBeenCalledWith(
            expect.stringContaining('live.mic_permission_denied'),
            expect.anything()
        );
    });
});
