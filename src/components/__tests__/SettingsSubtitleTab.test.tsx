import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsSubtitleTab } from '../settings/SettingsSubtitleTab';

// Mock translation
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

const mockUpdateConfig = vi.fn();
const mockReadiness = vi.hoisted(() => ({
    state: 'ready' as 'ready' | 'failed',
    lastErrorSource: null as null | 'shortcut_registration' | 'warmup' | 'microphone' | 'session',
    lastErrorMessage: null as string | null,
}));

vi.mock('../../hooks/useVoiceTypingReadiness', () => ({
    useVoiceTypingReadiness: () => mockReadiness,
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

vi.mock('../settings/SettingsShortcutInput', () => ({
    SettingsShortcutInput: ({ value, onChange }: any) => (
        <input
            aria-label="voice typing shortcut"
            value={value}
            onChange={(event) => onChange?.(event.target.value)}
        />
    ),
}));

vi.mock('../../stores/configStore', () => ({
    useCaptionConfig: () => ({
        lockWindow: false,
        alwaysOnTop: true,
        startOnLaunch: false,
        captionWindowWidth: 800,
        captionFontSize: 24,
        captionFontColor: '#ffffff',
        captionBackgroundOpacity: 0.6,
    }),
    useVoiceTypingConfig: () => ({
        voiceTypingEnabled: false,
        voiceTypingShortcut: 'Alt+V',
        voiceTypingMode: 'hold',
    }),
    useSetConfig: () => mockUpdateConfig,
}));

describe('SettingsSubtitleTab', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mockReadiness.state = 'ready';
        mockReadiness.lastErrorSource = null;
        mockReadiness.lastErrorMessage = null;
    });

    it('renders all controls', () => {
        render(<SettingsSubtitleTab />);

        expect(screen.getByText('live.start_on_launch')).toBeDefined();
        expect(screen.getByText('live.lock_window')).toBeDefined();
        expect(screen.getByText('live.always_on_top')).toBeDefined();
        expect(screen.getByText('live.window_width')).toBeDefined();
        expect(screen.getByText('live.font_size')).toBeDefined();
        expect(screen.getByText('live.font_color')).toBeDefined();
        expect(screen.getByText('settings.enable_voice_typing')).toBeDefined();
        expect(screen.getByText('settings.voice_typing_shortcut')).toBeDefined();
        expect(screen.getByText('settings.voice_typing_mode')).toBeDefined();
        expect(screen.getByText('settings.voice_typing_availability')).toBeDefined();
    });

    it('renders width input with correct values and classes', () => {
        render(<SettingsSubtitleTab />);

        const numberInput = screen.getByDisplayValue('800');

        expect(numberInput).toBeDefined();
        expect(numberInput.tagName).toBe('INPUT');
        expect(numberInput.getAttribute('type')).toBe('number');
        expect(numberInput.classList.contains('settings-input')).toBe(true);

        // Ensure range slider is removed
        const widthRange = document.querySelector('input[type="range"][value="800"]');
        expect(widthRange).toBeNull();
    });

    it('calls updateConfig when inputs change', () => {
        render(<SettingsSubtitleTab />);

        const numberInput = screen.getByDisplayValue('800');

        fireEvent.change(numberInput, { target: { value: '1200' } });
        expect(mockUpdateConfig).toHaveBeenCalledWith({ captionWindowWidth: 1200 });
    });

    it('renders font size input with correct classes', () => {
        render(<SettingsSubtitleTab />);

        const numberInput = screen.getByDisplayValue('24');

        expect(numberInput).toBeDefined();
        expect(numberInput.tagName).toBe('INPUT');
        expect(numberInput.getAttribute('type')).toBe('number');
        expect(numberInput.classList.contains('settings-input')).toBe(true);

        // Ensure range slider is removed
        const fontRange = document.querySelector('input[type="range"][value="24"]');
        expect(fontRange).toBeNull();
    });

    it('renders color picker with correct structure and labels', () => {
        render(<SettingsSubtitleTab />);

        const colorInput = screen.getByLabelText('live.font_color');
        expect(colorInput.getAttribute('type')).toBe('color');
        expect(colorInput.getAttribute('value')).toBe('#ffffff');

        const textInput = screen.getByLabelText('live.font_color_hex');
        expect(textInput.getAttribute('type')).toBe('text');
        expect(textInput.getAttribute('value')).toBe('#ffffff');
        expect(textInput.classList.contains('settings-input')).toBe(true);
    });

    it('updates voice typing settings from the combined page', () => {
        render(<SettingsSubtitleTab />);

        const switches = screen.getAllByRole('switch');
        fireEvent.click(switches[3]);
        expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingEnabled: true });

        fireEvent.change(screen.getByLabelText('voice typing shortcut'), {
            target: { value: 'Ctrl+Alt+V' },
        });
        expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingShortcut: 'Ctrl+Alt+V' });

        fireEvent.change(document.querySelector('#vt-mode-select') as HTMLSelectElement, {
            target: { value: 'toggle' },
        });
        expect(mockUpdateConfig).toHaveBeenCalledWith({ voiceTypingMode: 'toggle' });
    });

    it('shows only simplified availability and the runtime failure reason', () => {
        mockReadiness.state = 'failed';
        mockReadiness.lastErrorSource = 'microphone';
        mockReadiness.lastErrorMessage = 'Microphone is unavailable.';

        render(<SettingsSubtitleTab />);

        expect(screen.getByText('settings.voice_typing_unavailable')).toBeDefined();
        expect(screen.getByText('settings.voice_typing_failure_reason_with_source')).toBeDefined();
        expect(screen.queryByText('settings.voice_typing_dependencies')).toBeNull();
        expect(screen.queryByText('settings.voice_typing_open_model_hub')).toBeNull();
        expect(screen.queryByText('settings.voice_typing_open_input_device')).toBeNull();
    });
});
