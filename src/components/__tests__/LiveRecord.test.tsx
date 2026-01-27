import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';

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
        isITNModelInstalled: vi.fn().mockResolvedValue(false),
        getITNModelPath: vi.fn().mockResolvedValue('/path/to/itn'),
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
    Pause: () => 'Pause',
    Play: () => 'Play',
    Square: () => 'Square',
    Mic: () => 'Mic',
    Monitor: () => 'Monitor',
    FileAudio: () => 'FileAudio',
}));

describe('LiveRecord', () => {
    beforeEach(() => {
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
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.clearAllMocks();
    });

    it('should increment timer correctly (not double speed)', async () => {
        render(<LiveRecord />);

        // Find the start button (it's the only button initially)
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
        });

        // Advance 3 seconds
        await act(async () => {
            vi.advanceTimersByTime(3000);
        });

        // Check text. formatTime(3) -> "00:03"
        const timeDisplay = screen.getByText(/00:0[0-9]/);

        console.log('Time Displayed:', timeDisplay.textContent);

        expect(timeDisplay.textContent).toBe('00:03');
    }, 10000);
});
