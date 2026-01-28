import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';
import { useTranscriptStore } from '../../stores/transcriptStore';

// Mock transcription service
vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        start: vi.fn((_seg, _err, onReady) => { if (onReady) onReady(); }),
        stop: vi.fn(),
        startSession: vi.fn(),
        sendAudioInt16: vi.fn(),
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),
        setVadModelPath: vi.fn(),
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

// Mock translation
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
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

describe('LiveRecord Performance', () => {
    let rafSpy: any;
    let cancelRafSpy: any;
    let rafCallback: FrameRequestCallback | null = null;

    beforeEach(() => {
        vi.useFakeTimers();

        // Mock requestAnimationFrame to just capture the callback
        // We control execution manually
        rafSpy = vi.fn((cb) => {
            rafCallback = cb;
            return 123; // dummy ID
        });
        cancelRafSpy = vi.fn();

        vi.stubGlobal('requestAnimationFrame', rafSpy);
        vi.stubGlobal('cancelAnimationFrame', cancelRafSpy);

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
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
    });

    it('should stop rAF loop when paused and restart when resumed', async () => {
        act(() => {
            useTranscriptStore.setState({
                config: {
                    recognitionModelPath: '/fake/model',
                    vadModelPath: '/fake/vad',
                    enableITN: true,
                    language: 'en',
                    appLanguage: 'auto',
                    punctuationModelPath: '',
                    theme: 'auto',
                    font: 'system'
                }
            });
        });

        render(<LiveRecord />);

        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        // Start recording
        await act(async () => {
            fireEvent.click(startBtn);
        });

        // Advance timers to allow async startRecording to finish
        await act(async () => {
            await Promise.resolve(); // flush microtasks
        });

        // At this point, drawVisualizer should have been called, initiating the loop
        expect(rafSpy).toHaveBeenCalled();
        rafSpy.mockClear();

        // Simulate one frame
        await act(async () => {
             if (rafCallback) {
                 rafCallback(performance.now());
             }
        });

        // Should request next frame
        expect(rafSpy).toHaveBeenCalledTimes(1);
        rafSpy.mockClear();

        // Pause recording
        const pauseBtn = screen.getByLabelText(/live.pause/i);
        await act(async () => {
            fireEvent.click(pauseBtn);
        });

        // Now we are paused.
        // CURRENT BEHAVIOR: The loop is still scheduled.
        // Simulate the frame that was scheduled BEFORE pause (or during pause if logic is flawed)

        // Reset spy to check what happens next
        rafSpy.mockClear();

        await act(async () => {
             if (rafCallback) {
                 rafCallback(performance.now());
             }
        });

        // With optimization, this should match 0.
        // Without optimization, it matches 1.
        console.log('rAF calls during pause:', rafSpy.mock.calls.length);

        expect(rafSpy).toHaveBeenCalledTimes(0);

        // Resume
        const resumeBtn = screen.getByLabelText(/live.resume/i);
        await act(async () => {
            fireEvent.click(resumeBtn);
        });

        rafSpy.mockClear();

        // Check that loop restarted
        await act(async () => {
             if (rafCallback) {
                 rafCallback(performance.now());
             }
        });
        expect(rafSpy).toHaveBeenCalled();
    });
});
