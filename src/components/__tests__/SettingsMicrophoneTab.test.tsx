import React from 'react';
import { render, waitFor } from '@testing-library/react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { SettingsMicrophoneTab } from '../settings/SettingsMicrophoneTab';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { useTranscriptStore } from '../../stores/transcriptStore';

const mockInvoke = vi.fn();
const mockListen = vi.fn();
const mockRemove = vi.fn();

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
    listMicrophoneDeviceOptions: vi.fn().mockResolvedValue([
        { label: 'Auto', value: 'default' },
    ]),
    listSystemAudioDeviceOptions: vi.fn().mockResolvedValue([
        { label: 'Auto', value: 'default' },
    ]),
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
        mockInvoke.mockImplementation(async (command: string) => {
            if (command === 'stop_microphone_capture' || command === 'stop_system_audio_capture') {
                return '';
            }
            return undefined;
        });
        mockListen.mockResolvedValue(() => {});
        mockRemove.mockResolvedValue(undefined);

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
});
