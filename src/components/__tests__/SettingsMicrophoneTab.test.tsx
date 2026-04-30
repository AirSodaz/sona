import { act, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsMicrophoneTab } from '../settings/SettingsMicrophoneTab';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';

const mockInvoke = vi.fn();
const mockListen = vi.fn();
const mockRemove = vi.fn();
const mockListMicrophoneDeviceOptions = vi.fn();
const mockListSystemAudioDeviceOptions = vi.fn();

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

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

vi.mock('lucide-react', () => ({
    Volume2: () => null,
    SlidersHorizontal: () => null,
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
    Switch: () => <div />,
}));

vi.mock('../settings/SettingsLayout', () => ({
    SettingsTabContainer: ({ children }: any) => <div>{children}</div>,
    SettingsSection: ({ children }: any) => <section>{children}</section>,
    SettingsItem: ({ children }: any) => <div>{children}</div>,
    SettingsPageHeader: ({ title, description }: any) => (
        <header>
            <div>{title}</div>
            <div>{description}</div>
        </header>
    ),
}));

vi.mock('../../hooks/useAudioVisualizer', () => ({
    useAudioVisualizer: () => ({
        startVisualizer: vi.fn(),
        stopVisualizer: vi.fn(),
    }),
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

        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'stop_microphone_capture' || command === 'stop_system_audio_capture') {
                return '';
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
            },
        });
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
});
