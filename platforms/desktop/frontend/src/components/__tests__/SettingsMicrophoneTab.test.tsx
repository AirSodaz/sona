import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsMicrophoneTab } from '../settings/SettingsMicrophoneTab';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { useVoiceTypingRuntimeStore } from '../../stores/voiceTypingRuntimeStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';

const mockInvoke = vi.fn();
const mockListen = vi.fn();
const mockRemove = vi.fn();
const mockListMicrophoneDeviceOptions = vi.fn();
const mockListSystemAudioDeviceOptions = vi.fn();
const mockVisualizerPeakRefs = vi.hoisted(() => [] as Array<{ current: number }>);
const mockOpen = vi.fn();

function createDeferred<T>() {
    let resolve!: (value: T | PromiseLike<T>) => void;
    const promise = new Promise<T>((res) => {
        resolve = res;
    });
    return { promise, resolve };
}

function getInvokeCalls(command: string) {
    return mockInvoke.mock.calls.filter(([calledCommand]) => calledCommand === command);
}

function getListenCalls(eventName: string) {
    return mockListen.mock.calls.filter(([calledEventName]) => calledEventName === eventName);
}

vi.mock('@tauri-apps/api/core', () => ({
    invoke: (command: string, args?: unknown) => mockInvoke(command, args),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: (event: string, callback: unknown) => mockListen(event, callback),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    remove: (path: string) => mockRemove(path),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: (...args: unknown[]) => mockOpen(...args),
}));

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

vi.mock('lucide-react', () => ({
    Volume2: () => null,
    SlidersHorizontal: () => null,
    FileAudio: () => null,
    FolderOpen: () => null,
    ExternalLink: () => null,
    RotateCcw: () => null,
}));

vi.mock('../Icons', () => ({
    MicIcon: () => null,
}));

vi.mock('../Dropdown', () => ({
    Dropdown: ({ id, value, onChange, options }: any) => (
        <select id={id} value={value} onChange={(event) => onChange?.(event.target.value)}>
            {options?.map((option: any) => (
                <option key={option.value} value={option.value}>
                    {option.label}
                </option>
            ))}
        </select>
    ),
}));

vi.mock('../Switch', () => ({
    Switch: ({ checked, onChange, label, 'aria-label': ariaLabel }: any) => (
        <button
            type="button"
            role="switch"
            aria-checked={checked}
            aria-label={ariaLabel || label}
            onClick={() => onChange?.(!checked)}
        >
            {label || ariaLabel || 'switch'}
        </button>
    ),
}));

vi.mock('../settings/SettingsLayout', async (importOriginal) => {
    const actual = await importOriginal<typeof import('../settings/SettingsLayout')>();
    return {
        ...actual,
        SettingsTabContainer: ({ children }: any) => <div>{children}</div>,
        SettingsSection: ({ children }: any) => <section>{children}</section>,
        SettingsItem: ({ children }: any) => <div>{children}</div>,
        SettingsPageHeader: ({ title, description }: any) => (
            <header>
                <div>{title}</div>
                <div>{description}</div>
            </header>
        ),
    };
});

vi.mock('../../hooks/useAudioVisualizer', () => ({
    useAudioVisualizer: ({ peakLevelRef }: { peakLevelRef: { current: number } }) => {
        mockVisualizerPeakRefs.push(peakLevelRef);
        return {
            startVisualizer: vi.fn(),
            stopVisualizer: vi.fn(),
        };
    },
}));

vi.mock('../../services/audioDeviceService', () => ({
    listMicrophoneDeviceOptions: (...args: unknown[]) => mockListMicrophoneDeviceOptions(...args),
    listSystemAudioDeviceOptions: (...args: unknown[]) => mockListSystemAudioDeviceOptions(...args),
}));

vi.mock('../../utils/logger', () => ({
    logger: {
        error: vi.fn(),
        warn: vi.fn(),
        info: vi.fn(),
        debug: vi.fn(),
    },
}));

describe('SettingsMicrophoneTab', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockVisualizerPeakRefs.length = 0;

        vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
            callback(16);
            return 1;
        }));
        vi.stubGlobal('cancelAnimationFrame', vi.fn());
        Object.defineProperty(window, 'requestAnimationFrame', {
            configurable: true,
            writable: true,
            value: globalThis.requestAnimationFrame,
        });
        Object.defineProperty(window, 'cancelAnimationFrame', {
            configurable: true,
            writable: true,
            value: globalThis.cancelAnimationFrame,
        });

        mockInvoke.mockImplementation(async (command: string, args?: any) => {
            if (command === 'stop_microphone_capture' || command === 'stop_system_audio_capture') {
                return '';
            }
            if (command === 'get_runtime_environment_status') {
                return {
                    ffmpegPath: 'C:\\app\\ffmpeg.exe',
                    ffmpegExists: true,
                    logDirPath: 'C:\\app\\logs',
                };
            }
            if (command === 'get_path_statuses') {
                const paths: string[] = args?.paths || [];
                return paths.map((p) => ({
                    path: p,
                    kind: p.includes('missing') ? 'missing' : 'file',
                    error: null,
                }));
            }
            return undefined;
        });
        mockListen.mockResolvedValue(() => {});
        mockRemove.mockResolvedValue(undefined);
        mockListMicrophoneDeviceOptions.mockResolvedValue([
            { label: 'Auto', value: 'default' },
        ]);
        mockListSystemAudioDeviceOptions.mockResolvedValue([
            { label: 'Auto', value: 'default' },
        ]);

        useConfigStore.setState({
            config: {
                ...DEFAULT_CONFIG,
                microphoneId: 'default',
                systemAudioDeviceId: 'default',
                microphoneBoost: 1,
                keepMicrophoneActive: false,
            },
        });
        useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
        useTranscriptStore.setState({
            isRecording: false,
            isCaptionMode: false,
            isPaused: false,
            segments: [],
        } as Partial<ReturnType<typeof useTranscriptStore.getState>>);
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    it('does not enumerate devices or start previews while inactive for tab prewarm', async () => {
        render(<SettingsMicrophoneTab isActiveTab={false} isOpen />);

        await act(async () => {
            await Promise.resolve();
        });

        expect(mockListMicrophoneDeviceOptions).not.toHaveBeenCalled();
        expect(mockListSystemAudioDeviceOptions).not.toHaveBeenCalled();
        expect(getInvokeCalls('start_microphone_capture')).toHaveLength(0);
        expect(getInvokeCalls('start_system_audio_capture')).toHaveLength(0);
    });

    it('stops the microphone visualizer with the test_mic instance id on cleanup', async () => {
        const { unmount } = render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('start_microphone_capture', {
                deviceName: null,
                instanceId: 'test_mic',
            });
        });

        unmount();

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('stop_microphone_capture', {
                instanceId: 'test_mic',
            });
        });
    });

    it('does not start preview capture before both device lists finish loading', async () => {
        const microphoneDevicesDeferred = createDeferred<{ label: string; value: string }[]>();
        const systemDevicesDeferred = createDeferred<{ label: string; value: string }[]>();

        mockListMicrophoneDeviceOptions.mockReturnValueOnce(microphoneDevicesDeferred.promise);
        mockListSystemAudioDeviceOptions.mockReturnValueOnce(systemDevicesDeferred.promise);

        const { unmount } = render(<SettingsMicrophoneTab isActiveTab isOpen />);

        expect(getInvokeCalls('start_microphone_capture')).toHaveLength(0);
        expect(getInvokeCalls('start_system_audio_capture')).toHaveLength(0);

        unmount();

        await act(async () => {
            microphoneDevicesDeferred.resolve([{ label: 'Auto', value: 'default' }]);
            systemDevicesDeferred.resolve([{ label: 'Auto', value: 'default' }]);
            await Promise.all([microphoneDevicesDeferred.promise, systemDevicesDeferred.promise]);
        });

        expect(getInvokeCalls('start_microphone_capture')).toHaveLength(0);
        expect(getInvokeCalls('start_system_audio_capture')).toHaveLength(0);
    });

    it('starts microphone preview before system-audio preview after device loading completes', async () => {
        render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('start_microphone_capture', {
                deviceName: null,
                instanceId: 'test_mic',
            });
        });

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('start_system_audio_capture', {
                deviceName: null,
                instanceId: 'test_system',
            });
        });

        const microphoneStartIndex = mockInvoke.mock.calls.findIndex(([command]) => command === 'start_microphone_capture');
        const systemStartIndex = mockInvoke.mock.calls.findIndex(([command]) => command === 'start_system_audio_capture');

        expect(microphoneStartIndex).toBeGreaterThanOrEqual(0);
        expect(systemStartIndex).toBeGreaterThan(microphoneStartIndex);
    });

    it('reuses an existing persistent voice typing microphone capture for preview metering', async () => {
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                voiceTypingEnabled: true,
                keepMicrophoneActive: true,
            },
        });
        useVoiceTypingRuntimeStore.getState().setWarmupStatus('ready');

        render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(mockListen).toHaveBeenCalledWith('microphone-audio', expect.any(Function));
        });

        expect(getInvokeCalls('start_microphone_capture')).toEqual([]);
    });

    it('applies microphone boost to the preview waveform peak', async () => {
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                microphoneBoost: 2,
            },
        });

        render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(getListenCalls('microphone-audio')).toHaveLength(1);
        });

        const listener = getListenCalls('microphone-audio')[0]?.[1] as
            | ((event: { payload: number }) => void)
            | undefined;
        expect(listener).toBeDefined();

        act(() => {
            listener?.({ payload: 8192 });
        });

        expect(mockVisualizerPeakRefs[0]?.current).toBeCloseTo((8192 / 32767) * 2);
    });

    it('renders a global keep-microphone-active switch that updates audio config', async () => {
        render(<SettingsMicrophoneTab isActiveTab isOpen={false} />);

        const toggle = screen.getByRole('switch', {
            name: 'settings.keep_microphone_active',
        });

        expect(toggle.getAttribute('aria-checked')).toBe('false');

        await act(async () => {
            fireEvent.click(toggle);
        });

        expect(useConfigStore.getState().config.keepMicrophoneActive).toBe(true);
    });

    it('cleans up a delayed microphone preview start after the page unmounts', async () => {
        const micStartDeferred = createDeferred<void>();

        mockInvoke.mockImplementation((command: string) => {
            if (command === 'start_microphone_capture') {
                return micStartDeferred.promise;
            }
            if (command === 'stop_microphone_capture' || command === 'stop_system_audio_capture') {
                return Promise.resolve('');
            }
            return Promise.resolve(undefined);
        });

        const { unmount } = render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('start_microphone_capture', {
                deviceName: null,
                instanceId: 'test_mic',
            });
        });
        expect(getListenCalls('microphone-audio')).toHaveLength(0);

        unmount();

        await act(async () => {
            micStartDeferred.resolve(undefined);
            await micStartDeferred.promise;
        });

        await waitFor(() => {
            expect(getInvokeCalls('stop_microphone_capture')).toEqual([
                ['stop_microphone_capture', { instanceId: 'test_mic' }],
            ]);
        });
        expect(getListenCalls('microphone-audio')).toHaveLength(0);
    });

    it('cleans up a delayed system-audio preview start after the page unmounts', async () => {
        const systemStartDeferred = createDeferred<void>();

        mockInvoke.mockImplementation((command: string) => {
            if (command === 'start_system_audio_capture') {
                return systemStartDeferred.promise;
            }
            if (command === 'stop_microphone_capture' || command === 'stop_system_audio_capture') {
                return Promise.resolve('');
            }
            return Promise.resolve(undefined);
        });

        const { unmount } = render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('start_microphone_capture', {
                deviceName: null,
                instanceId: 'test_mic',
            });
        });

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('start_system_audio_capture', {
                deviceName: null,
                instanceId: 'test_system',
            });
        });
        expect(getListenCalls('system-audio')).toHaveLength(0);

        unmount();

        await act(async () => {
            systemStartDeferred.resolve(undefined);
            await systemStartDeferred.promise;
        });

        await waitFor(() => {
            expect(getInvokeCalls('stop_system_audio_capture')).toEqual([
                ['stop_system_audio_capture', { instanceId: 'test_system' }],
            ]);
        });
        expect(getListenCalls('system-audio')).toHaveLength(0);
    });

    it('renders the FFmpeg section with default sidecar path and allows browsing a custom path', async () => {
        mockOpen.mockResolvedValue('D:\\tools\\ffmpeg.exe');

        render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(screen.getByText('C:\\app\\ffmpeg.exe')).toBeDefined();
            expect(screen.getByText('common.default')).toBeDefined();
            expect(screen.getByText('common.ready')).toBeDefined();
        });

        const browseBtn = screen.getByText('common.change_path');
        fireEvent.click(browseBtn);

        await waitFor(() => {
            expect(mockOpen).toHaveBeenCalledWith({
                multiple: false,
                directory: false,
                filters: [
                    {
                        name: 'FFmpeg Executable',
                        extensions: ['exe', '*'],
                    },
                ],
            });
            expect(useConfigStore.getState().config.ffmpegPath).toBe('D:\\tools\\ffmpeg.exe');
        });
    });

    it('renders custom badge and allows restoring default when custom ffmpeg path is set', async () => {
        useConfigStore.setState({
            config: {
                ...DEFAULT_CONFIG,
                ffmpegPath: 'D:\\custom\\ffmpeg.exe',
            },
        });

        render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(screen.getByText('D:\\custom\\ffmpeg.exe')).toBeDefined();
            expect(screen.getByText('common.custom')).toBeDefined();
            expect(screen.getByText('common.restore_default')).toBeDefined();
        });

        const restoreBtn = screen.getByText('common.restore_default');
        fireEvent.click(restoreBtn);

        expect(useConfigStore.getState().config.ffmpegPath).toBe('');
    });

    it('opens the FFmpeg folder when clicking open folder button', async () => {
        render(<SettingsMicrophoneTab isActiveTab isOpen />);

        await waitFor(() => {
            expect(screen.getByText('common.open_folder')).toBeDefined();
        });

        const openFolderBtn = screen.getByText('common.open_folder');
        fireEvent.click(openFolderBtn);

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith('storage_open_path', {
                path: 'C:\\app\\ffmpeg.exe',
            });
        });
    });
});
