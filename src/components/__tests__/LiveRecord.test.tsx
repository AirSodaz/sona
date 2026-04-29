import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';
import { useOnboardingStore } from '../../stores/onboardingStore';
import type { HistoryItem } from '../../types/history';
import type { LiveRecordingDraftHandle } from '../../services/historyService';

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

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(() => { }),
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

function createDraftHandle(
    id: string,
    extension = 'wav',
    overrides: Partial<HistoryItem> = {},
): LiveRecordingDraftHandle {
    return {
        item: {
            id,
            timestamp: 1,
            duration: 0,
            audioPath: `${id}.${extension}`,
            transcriptPath: `${id}.json`,
            title: `Recording ${id}`,
            previewText: '',
            projectId: null,
            icon: 'system:mic',
            type: 'recording',
            searchContent: '',
            status: 'draft',
            draftSource: 'live_record',
            ...overrides,
        },
        audioAbsolutePath: `C:/mock/history/${id}.${extension}`,
    };
}

function createCompletedHistoryItem(
    id: string,
    extension = 'wav',
    overrides: Partial<HistoryItem> = {},
): HistoryItem {
    return {
        id,
        timestamp: 1,
        duration: 1,
        audioPath: `${id}.${extension}`,
        transcriptPath: `${id}.json`,
        title: `Recording ${id}`,
        previewText: '',
        projectId: null,
        icon: 'system:mic',
        type: 'recording',
        searchContent: '',
        status: 'complete',
        ...overrides,
    };
}

// Mock history service
const mockSaveRecording = vi.fn().mockResolvedValue({ id: 'test-id', title: 'Recording test', projectId: null });
const mockSaveNativeRecording = vi.fn().mockResolvedValue({ id: 'test-id', title: 'Recording test', projectId: null });
const mockCreateLiveRecordingDraft = vi.fn();
const mockCompleteLiveRecordingDraft = vi.fn();
const mockDeleteRecording = vi.fn().mockResolvedValue(undefined);
let liveDraftCounter = 0;
const liveDraftHandles = new Map<string, LiveRecordingDraftHandle>();

vi.mock('../../services/historyService', () => ({
    historyService: {
        createLiveRecordingDraft: (...args: any[]) => mockCreateLiveRecordingDraft(...args),
        completeLiveRecordingDraft: (...args: any[]) => mockCompleteLiveRecordingDraft(...args),
        discardLiveRecordingDraft: (...args: any[]) => mockDeleteRecording(...args),
        deleteRecording: (...args: any[]) => mockDeleteRecording(...args),
        updateTranscript: vi.fn().mockResolvedValue(undefined),
        saveRecording: (blob: Blob, segments: any, duration: number) => mockSaveRecording(blob, segments, duration),
        saveNativeRecording: (path: string, segments: any, duration: number) => mockSaveNativeRecording(path, segments, duration),
        saveImportedFile: vi.fn().mockResolvedValue({ id: 'test-id' }),
        getAll: vi.fn().mockResolvedValue([]),
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
    writeFile: vi.fn(() => Promise.resolve()),
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

describe('LiveRecord', () => {
    let capturedOnSegment: any = null;

    beforeEach(async () => {
        vi.useFakeTimers();
        capturedOnSegment = null;
        liveDraftCounter = 0;
        liveDraftHandles.clear();
        localStorage.clear();
        mockInvoke.mockImplementation(async (cmd: string) => {
            if (cmd === 'stop_system_audio_capture' || cmd === 'stop_microphone_capture') {
                return '/mock/path/to/audio.wav';
            }
            return undefined;
        });
        mockCreateLiveRecordingDraft.mockReset();
        mockCompleteLiveRecordingDraft.mockReset();
        mockDeleteRecording.mockReset();
        mockDeleteRecording.mockImplementation(async (historyId: string) => {
            liveDraftHandles.delete(historyId);
        });
        mockCreateLiveRecordingDraft.mockImplementation(async (audioExtension: string, projectId?: string | null, icon?: string | null) => {
            liveDraftCounter += 1;
            const draft = createDraftHandle(`draft-${liveDraftCounter}`, audioExtension, {
                projectId: projectId ?? null,
                icon: icon ?? 'system:mic',
            });
            liveDraftHandles.set(draft.item.id, draft);
            return draft;
        });
        mockCompleteLiveRecordingDraft.mockImplementation(async (historyId: string, segments: Array<{ text?: string }>, duration: number) => {
            const draft = liveDraftHandles.get(historyId) ?? createDraftHandle(historyId);
            return createCompletedHistoryItem(historyId, draft.item.audioPath.split('.').pop() || 'wav', {
                title: draft.item.title,
                icon: draft.item.icon,
                projectId: draft.item.projectId,
                duration,
                previewText: segments[0]?.text || '',
                searchContent: segments.map((segment) => segment.text || '').join(' ').trim(),
            });
        });

        mockStart.mockImplementation((onSeg: any) => {
            capturedOnSegment = onSeg;
            return Promise.resolve();
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
        const { useConfigStore } = await import('../../stores/configStore');
        act(() => {
            useConfigStore.setState({
                config: {
                    ...useConfigStore.getState().config, streamingModelPath: "/path/to/model",
                    offlineModelPath: '/path/to/model'
                }
            });
            useOnboardingStore.setState({
                persistedState: { version: 1, status: 'pending' },
                currentStep: 'welcome',
                entryContext: 'startup',
                isOpen: false,
                focusStartRecordingToken: 0,
            });
        });

        // Reset mocks
        mockStart.mockClear();
        mockStop.mockClear();
        mockSoftStop.mockClear();
        mockPauseStream.mockClear();
        mockResumeStream.mockClear();
        mockPrepare.mockClear();
    });

    afterEach(async () => {
        localStorage.clear();
        // Reset store state
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        act(() => {
            useTranscriptStore.setState({
                isRecording: false,
                isPaused: false,
                isCaptionMode: false,
                segments: [],
                audioUrl: null,
                sourceHistoryId: null,
                title: null,
                icon: null,
            });
            useOnboardingStore.setState({
                persistedState: { version: 1, status: 'pending' },
                currentStep: 'welcome',
                entryContext: 'startup',
                isOpen: false,
                focusStartRecordingToken: 0,
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
        expect(useTranscriptStore.getState().sourceHistoryId).toBe('draft-1');
        expect(mockCreateLiveRecordingDraft).toHaveBeenCalledWith('wav', null, 'system:mic');
    });

    it('pauses native recording by flushing the current final segment and blocks later partials until resume', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        mockPauseStream.mockImplementationOnce(async () => {
            capturedOnSegment?.({
                id: 'pause-final',
                text: 'Paused final segment',
                start: 0,
                end: 1,
                isFinal: true
            });
        });

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await Promise.resolve();
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.pause/i }));
            await Promise.resolve();
        });

        expect(mockInvoke).toHaveBeenCalledWith('set_microphone_capture_paused', { instanceId: 'record', paused: true });
        expect(mockPauseStream).toHaveBeenCalled();
        expect(useTranscriptStore.getState().isPaused).toBe(true);
        expect(useTranscriptStore.getState().segments).toEqual([
            expect.objectContaining({
                id: 'pause-final',
                text: 'Paused final segment',
                isFinal: true
            })
        ]);

        await act(async () => {
            capturedOnSegment?.({
                id: 'should-drop',
                text: 'Should not be accepted while paused',
                start: 1,
                end: 2,
                isFinal: false
            });
            await Promise.resolve();
        });

        expect(useTranscriptStore.getState().segments).toHaveLength(1);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.resume/i }));
            await Promise.resolve();
        });

        expect(mockResumeStream).toHaveBeenCalled();
        expect(mockInvoke).toHaveBeenCalledWith('set_microphone_capture_paused', { instanceId: 'record', paused: false });
        expect(useTranscriptStore.getState().isPaused).toBe(false);
    });

    it('keeps transcript segment timestamps monotonic across pause and resume', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        mockPauseStream.mockImplementationOnce(async () => {
            capturedOnSegment?.({
                id: 'pause-final',
                text: 'Pause final segment',
                start: 0,
                end: 2,
                isFinal: true,
                timestamps: [0, 1]
            });
        });

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(2000);
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.pause/i }));
            await Promise.resolve();
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.resume/i }));
            await Promise.resolve();
        });

        await act(async () => {
            capturedOnSegment?.({
                id: 'resume-seg',
                text: 'After resume',
                start: 0,
                end: 0.5,
                isFinal: true,
                timestamps: [0, 0.25]
            });
            await Promise.resolve();
        });

        const resumedSegment = useTranscriptStore.getState().segments.find(segment => segment.id === 'resume-seg');
        expect(resumedSegment).toBeTruthy();
        expect(resumedSegment?.start).toBeGreaterThanOrEqual(2);
        expect(resumedSegment?.start).toBeLessThan(2.2);
        expect(resumedSegment?.end).toBeGreaterThanOrEqual(2.5);
        expect(resumedSegment?.end).toBeLessThan(2.7);
        expect(resumedSegment?.timestamps?.[0]).toBeGreaterThanOrEqual(2);
        expect(resumedSegment?.timestamps?.[0]).toBeLessThan(2.2);
        expect(resumedSegment?.timestamps?.[1]).toBeGreaterThanOrEqual(2.25);
        expect(resumedSegment?.timestamps?.[1]).toBeLessThan(2.45);
    });

    it('excludes paused time from the saved native recording duration', async () => {
        mockPauseStream.mockImplementationOnce(async () => {
            capturedOnSegment?.({
                id: 'duration-final',
                text: 'Duration anchor',
                start: 0,
                end: 1,
                isFinal: true
            });
        });

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(1);
        });

        await act(async () => {
            await vi.advanceTimersByTimeAsync(2000);
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.pause/i }));
            await vi.advanceTimersByTimeAsync(1);
        });

        await act(async () => {
            await vi.advanceTimersByTimeAsync(5000);
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.resume/i }));
            await vi.advanceTimersByTimeAsync(1);
        });

        await act(async () => {
            await vi.advanceTimersByTimeAsync(1000);
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.stop/i }));
            await vi.advanceTimersByTimeAsync(1);
        });

        expect(mockCompleteLiveRecordingDraft).toHaveBeenCalled();
        const lastCall = mockCompleteLiveRecordingDraft.mock.calls[mockCompleteLiveRecordingDraft.mock.calls.length - 1];
        const duration = lastCall?.[2];
        expect(duration).toBeGreaterThanOrEqual(2.9);
        expect(duration).toBeLessThan(3.2);
    });

    it('keeps the displayed timer accumulated across pause and resume when using web audio fallback', async () => {
        mockInvoke.mockImplementation(async (cmd: string) => {
            if (cmd === 'start_microphone_capture') {
                throw new Error('native unavailable');
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

        const timer = screen.getByRole('timer');

        await act(async () => {
            await vi.advanceTimersByTimeAsync(2000);
        });
        expect(timer.textContent).toBe('00:02');

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.pause/i }));
            await vi.advanceTimersByTimeAsync(1);
        });
        expect(timer.textContent).toBe('00:02');

        await act(async () => {
            await vi.advanceTimersByTimeAsync(5000);
        });
        expect(timer.textContent).toBe('00:02');

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.resume/i }));
            await vi.advanceTimersByTimeAsync(1000);
        });
        expect(timer.textContent).toBe('00:03');
    });

    it('syncs the saved history title into the editor after web recording fallback is persisted', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        mockInvoke.mockImplementation(async (cmd: string) => {
            if (cmd === 'start_microphone_capture') {
                throw new Error('native unavailable');
            }
            if (cmd === 'stop_system_audio_capture' || cmd === 'stop_microphone_capture') {
                return '/mock/path/to/audio.wav';
            }
            return undefined;
        });
        mockCreateLiveRecordingDraft
            .mockResolvedValueOnce(createDraftHandle('native-fallback-draft', 'wav'))
            .mockResolvedValueOnce(createDraftHandle('web-history-id', 'webm'));
        mockCompleteLiveRecordingDraft.mockResolvedValueOnce(createCompletedHistoryItem('web-history-id', 'webm', {
            id: 'web-history-id',
            title: 'Recording 2026-04-27 09-00-00',
            icon: 'system:mic',
            projectId: null,
        }));

        render(<LiveRecord />);

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.start_recording/i }));
            await vi.advanceTimersByTimeAsync(100);
        });

        act(() => {
            useTranscriptStore.setState({
                segments: [{ id: 'web-seg', text: 'Hello web fallback', start: 0, end: 1, isFinal: true }],
            });
        });

        await act(async () => {
            fireEvent.click(screen.getByRole('button', { name: /live.stop/i }));
            await vi.advanceTimersByTimeAsync(100);
            await Promise.resolve();
        });

        expect(mockCompleteLiveRecordingDraft).toHaveBeenCalledWith(
            'web-history-id',
            expect.any(Array),
            expect.any(Number),
        );
        expect(useTranscriptStore.getState().sourceHistoryId).toBe('web-history-id');
        expect(useTranscriptStore.getState().title).toBe('Recording 2026-04-27 09-00-00');
        expect(useTranscriptStore.getState().icon).toBe('system:mic');
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

    it('reopens onboarding instead of starting recording when no live model is configured', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        const { useConfigStore } = await import('../../stores/configStore');
        act(() => {
            useConfigStore.setState({
                config: {
                    ...useConfigStore.getState().config,
                    streamingModelPath: '',
                    offlineModelPath: ''
                }
            });
        });

        render(<LiveRecord />);
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(50);
        });

        expect(useTranscriptStore.getState().isRecording).toBe(false);
        expect(useOnboardingStore.getState().isOpen).toBe(true);
        expect(useOnboardingStore.getState().currentStep).toBe('models');
    });

    it('should mute system audio when recording starts if configured', async () => {
        const { useConfigStore } = await import('../../stores/configStore');

        // Enable mute setting
        act(() => {
            useConfigStore.setState({
                config: {
                    ...useConfigStore.getState().config,
                    streamingModelPath: "/path/to/model",
                    offlineModelPath: '/path/to/model',
                    muteDuringRecording: true
                }
            });
        });

        render(<LiveRecord />);
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockInvoke).toHaveBeenCalledWith('set_system_audio_mute', { mute: true });

        // Stop recording
        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        await act(async () => {
            fireEvent.click(stopBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(mockInvoke).toHaveBeenCalledWith('set_system_audio_mute', { mute: false });
    });

    it('should process final segment emitted during softStop', async () => {
        // Mock softStop to emit a final segment
        mockSoftStop.mockImplementation(async () => {
            if (capturedOnSegment) {
                // Simulate waiting time
                await new Promise(resolve => setTimeout(resolve, 100));

                // Emit final segment
                capturedOnSegment({
                    id: 'final-seg',
                    text: 'Final segment text',
                    start: 0,
                    end: 1,
                    isFinal: true
                });
            }
        });

        render(<LiveRecord />);
        const startBtn = screen.getByRole('button', { name: /live.start_recording/i });

        await act(async () => {
            fireEvent.click(startBtn);
            await vi.advanceTimersByTimeAsync(100);
        });

        expect(capturedOnSegment).toBeTruthy();

        // Emit an initial segment
        await act(async () => {
            capturedOnSegment({
                id: 'seg1',
                text: 'Hello',
                start: 0,
                end: 0.5,
                isFinal: false
            });
        });

        const { useTranscriptStore } = await import('../../stores/transcriptStore');
        expect(useTranscriptStore.getState().segments).toHaveLength(1);

        // Stop recording
        const stopBtn = screen.getByRole('button', { name: /live.stop/i });
        await act(async () => {
            fireEvent.click(stopBtn);
            // softStop will wait 100ms then emit final segment
            await vi.advanceTimersByTimeAsync(200);
        });

        // Verify final segment is present
        const segments = useTranscriptStore.getState().segments;
        const lastSegment = segments[segments.length - 1];

        expect(mockSoftStop).toHaveBeenCalled();
        expect(lastSegment.text).toBe('Final segment text');
    });
});
