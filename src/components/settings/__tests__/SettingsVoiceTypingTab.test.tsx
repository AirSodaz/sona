import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SettingsSubtitleTab } from '../SettingsSubtitleTab';
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
    PRESET_MODELS_MAP: new Map(),
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

function renderCombinedSettings() {
    return render(<SettingsSubtitleTab />);
}

describe('SettingsSubtitleTab voice typing section', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setTestConfig();
        useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
    });

    it('shows unavailable when voice typing is disabled without dependency diagnostics', () => {
        renderCombinedSettings();

        expect(screen.getByText('Unavailable')).toBeDefined();
        expect(screen.queryByText('Readiness And Dependencies')).toBeNull();
        expect(screen.queryByRole('button', { name: 'Open Model Hub' })).toBeNull();
        expect(screen.queryByRole('button', { name: 'Open Input Device' })).toBeNull();
    });

    it('shows available when runtime registration and warm-up are complete', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: 'Alt + V',
            streamingModelPath: '/models/streaming-test-model',
            vadModelPath: '/models/vad.onnx',
        });
        useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
        useVoiceTypingRuntimeStore.getState().setWarmupStatus('ready');

        renderCombinedSettings();

        expect(screen.getByText('Available')).toBeDefined();
    });

    it('shows only the failed runtime reason when voice typing fails', () => {
        setTestConfig({
            voiceTypingEnabled: true,
            voiceTypingShortcut: 'Alt + V',
            streamingModelPath: '/models/streaming-test-model',
            vadModelPath: '/models/vad.onnx',
        });
        useVoiceTypingRuntimeStore
            .getState()
            .reportRuntimeError('microphone', 'Microphone is unavailable.');

        renderCombinedSettings();

        expect(screen.getByText('Unavailable')).toBeDefined();
        expect(screen.getByText('Failure reason: Input device: Microphone is unavailable.')).toBeDefined();
        expect(screen.queryByText('Voice Typing hit a runtime problem.')).toBeNull();
        expect(screen.queryByText('Source: Input device')).toBeNull();
        expect(screen.queryByRole('button', { name: 'Open Input Device' })).toBeNull();
    });
});
