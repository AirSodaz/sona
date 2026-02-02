import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';

// Spy on useTranslation to count renders
const { useTranslationSpy } = vi.hoisted(() => {
    return { useTranslationSpy: vi.fn(() => ({ t: (key: string) => key })) };
});

vi.mock('react-i18next', () => ({
    useTranslation: useTranslationSpy,
}));

// Mock transcription service
vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        start: vi.fn(),
        stop: vi.fn(),
        sendAudioInt16: vi.fn(),
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),
        setPunctuationModelPath: vi.fn(),
    }
}));

// Mock model service
vi.mock('../../services/modelService', () => ({
    modelService: {
        isITNModelInstalled: vi.fn().mockResolvedValue(true),
        getITNModelPath: vi.fn().mockResolvedValue('/path/to/itn'),
        getEnabledITNModelPaths: vi.fn().mockResolvedValue(['/path/to/itn']),
    }
}));

// Mock icons
vi.mock('lucide-react', () => ({
    Pause: () => <div data-testid="icon-pause" />,
    Play: () => <div data-testid="icon-play" />,
    Square: () => <div data-testid="icon-square" />,
    Mic: () => <div data-testid="icon-mic" />,
    Monitor: () => <div data-testid="icon-monitor" />,
    FileAudio: () => <div data-testid="icon-file-audio" />,
}));

describe('LiveRecord Render Performance', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        useTranslationSpy.mockClear();

        // Mock generic globals needed for LiveRecord
        vi.stubGlobal('requestAnimationFrame', vi.fn((cb) => setTimeout(cb, 16)));
        vi.stubGlobal('cancelAnimationFrame', vi.fn((id) => clearTimeout(id)));

        // Mock Media APIs
        vi.stubGlobal('MediaRecorder', class {
            state = 'inactive';
            stream: MediaStream;
            ondataavailable: ((e: any) => void) | null = null;
            onstop: (() => void) | null = null;
            mimeType = 'audio/webm';
            constructor(stream: MediaStream) { this.stream = stream; }
            start() { this.state = 'recording'; }
            stop() { this.state = 'inactive'; if (this.onstop) this.onstop(); }
            pause() { this.state = 'paused'; }
            resume() { this.state = 'recording'; }
            static isTypeSupported() { return true; }
        });

        vi.stubGlobal('AudioContext', class {
            state = 'running';
            destination = {};
            createMediaStreamSource() { return { connect: vi.fn() }; }
            createAnalyser() {
                return {
                    fftSize: 2048,
                    frequencyBinCount: 1024,
                    getByteFrequencyData: vi.fn(),
                    connect: vi.fn(),
                };
            }
            audioWorklet = { addModule: vi.fn().mockResolvedValue(undefined) };
            close() { return Promise.resolve(); }
        });

        vi.stubGlobal('AudioWorkletNode', class {
            port = { onmessage: null };
            connect = vi.fn();
        });

        if (!navigator.mediaDevices) {
            Object.defineProperty(navigator, 'mediaDevices', { value: {}, writable: true });
        }
        navigator.mediaDevices.getUserMedia = vi.fn().mockResolvedValue({
            getTracks: () => [{ stop: vi.fn() }],
        });

        HTMLCanvasElement.prototype.getContext = vi.fn(() => ({
            clearRect: vi.fn(),
            createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
            fillRect: vi.fn(),
            fillStyle: '',
        })) as any;
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
    });

    it('should count re-renders during recording', async () => {
        render(<LiveRecord />);

        // Initial render
        // const initialRenderCount = useTranslationSpy.mock.calls.length;

        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
        });

        // Wait for async start operations
        await act(async () => {
            await Promise.resolve();
        });

        const recordingStartRenderCount = useTranslationSpy.mock.calls.length;

        // Advance 3 seconds, 1 second at a time
        // This should trigger 3 interval updates, causing 3 re-renders in the unoptimized version
        for (let i = 0; i < 3; i++) {
            await act(async () => {
                vi.advanceTimersByTime(1000);
            });
        }

        const finalRenderCount = useTranslationSpy.mock.calls.length;

        const reRendersDuringRecording = finalRenderCount - recordingStartRenderCount;

        // With optimization, we expect 0 re-renders of the parent component
        expect(reRendersDuringRecording).toBe(0);

        // Verify that timer updated (implied by functionality, but good to check if we could access the timer text)
        // Since RecordingTimer is a child, we can check screen content
        const timeDisplay = screen.getByText(/00:03/);
        expect(timeDisplay).toBeDefined();
    });
});
