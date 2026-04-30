import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SettingsVoiceTypingTab } from '../SettingsVoiceTypingTab';
import { SettingsNavigationProvider } from '../SettingsNavigationContext';
import { useVoiceTypingRuntimeStore } from '../../../stores/voiceTypingRuntimeStore';
import { setTestConfig } from '../../../test-utils/configTestUtils';

function interpolate(template: string, options?: Record<string, unknown>) {
    return template.replace(/\{\{(\w+)\}\}/g, (_, key) => String(options?.[key] ?? ''));
}

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string, options?: Record<string, unknown>) => {
            if (typeof options?.defaultValue === 'string') {
                return interpolate(options.defaultValue, options);
            }

            return key;
        },
    }),
}));

vi.mock('../../../services/modelService', () => ({
    PRESET_MODELS: [
        {
            id: 'streaming-test-model',
            filename: 'streaming-test-model',
            modes: ['streaming'],
        },
    ],
    modelService: {
        getModelRules: vi.fn(() => ({
            requiresVad: true,
            requiresPunctuation: false,
        })),
    },
}));

vi.mock('../../Dropdown', () => ({
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

vi.mock('../../Switch', () => ({
    Switch: ({ checked, onChange }: any) => (
        <button
            type="button"
            role="switch"
            aria-checked={checked}
            onClick={() => onChange?.(!checked)}
        >
            {checked ? 'on' : 'off'}
        </button>
    ),
}));

function renderVoiceTypingTab() {
    return render(
        <SettingsNavigationProvider
            value={{
                activeTab: 'voice_typing',
                navigateToTab: vi.fn(),
            }}
        >
            <SettingsVoiceTypingTab />
        </SettingsNavigationProvider>
    );
}

describe('SettingsVoiceTypingTab', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setTestConfig();
        useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
    });

    it('shows the off state when voice typing is disabled', () => {
        renderVoiceTypingTab();

        expect(
            screen.getByText('Voice Typing is currently turned off.')
        ).toBeDefined();
    });

    it('shows the missing shortcut state when enabled without a shortcut', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: '   ',
            streamingModelPath: '/models/streaming-test-model',
            vadModelPath: '/models/vad.onnx',
        });
        useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
        useVoiceTypingRuntimeStore.getState().setWarmupStatus('ready');

        renderVoiceTypingTab();

        expect(
            screen.getByText('Voice Typing needs a shortcut before it can start.')
        ).toBeDefined();
    });

    it('shows the missing live model state when enabled without a model', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: 'Alt + V',
            streamingModelPath: '',
        });
        useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
        useVoiceTypingRuntimeStore.getState().setWarmupStatus('ready');

        renderVoiceTypingTab();

        expect(screen.getByText('Voice Typing needs a Live Record Model.')).toBeDefined();
        expect(screen.getByRole('button', { name: 'Open Model Hub' })).toBeDefined();
    });

    it('shows the missing VAD state when the selected model requires VAD', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: 'Alt + V',
            streamingModelPath: '/models/streaming-test-model',
            vadModelPath: '',
        });
        useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
        useVoiceTypingRuntimeStore.getState().setWarmupStatus('ready');

        renderVoiceTypingTab();

        expect(
            screen.getByText('The selected Live Record Model also needs a VAD model.')
        ).toBeDefined();
        expect(screen.getByRole('button', { name: 'Open Model Hub' })).toBeDefined();
    });

    it('shows the preparing state while registration and warm-up are still settling', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: 'Alt + V',
            streamingModelPath: '/models/streaming-test-model',
            vadModelPath: '/models/vad.onnx',
        });

        renderVoiceTypingTab();

        expect(
            screen.getByText('Voice Typing is getting ready in the background.')
        ).toBeDefined();
    });

    it('shows the ready state when runtime registration and warm-up are complete', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: 'Alt + V',
            streamingModelPath: '/models/streaming-test-model',
            vadModelPath: '/models/vad.onnx',
        });
        useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
        useVoiceTypingRuntimeStore.getState().setWarmupStatus('ready');

        renderVoiceTypingTab();

        expect(
            screen.getByText('Voice Typing is ready to dictate into other apps.')
        ).toBeDefined();
    });

    it('shows the failed state with the last observed error and remediation CTA', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: 'Alt + V',
            streamingModelPath: '/models/streaming-test-model',
            vadModelPath: '/models/vad.onnx',
        });
        useVoiceTypingRuntimeStore
            .getState()
            .reportRuntimeError('microphone', 'Microphone is unavailable.');

        renderVoiceTypingTab();

        expect(screen.getByText('Voice Typing hit a runtime problem.')).toBeDefined();
        expect(screen.getByText('Last error: Microphone is unavailable.')).toBeDefined();
        expect(screen.getByText('Source: Input device')).toBeDefined();
        expect(screen.getByRole('button', { name: 'Open Input Device' })).toBeDefined();
    });
});
