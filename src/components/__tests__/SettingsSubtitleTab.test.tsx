import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsSubtitleTab } from '../settings/SettingsSubtitleTab';
import { AppConfig } from '../../types/transcript';

// Mock translation
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

describe('SettingsSubtitleTab', () => {
    const mockUpdateConfig = vi.fn();
    const mockConfig: AppConfig = {
        offlineModelPath: '',
        language: 'auto',
        appLanguage: 'auto',
        lockWindow: false,
        alwaysOnTop: false,
        startOnLaunch: false,
        captionWindowWidth: 1000,
        captionFontSize: 24,
        captionFontColor: '#ffffff',
    };

    const defaultProps = {
        config: mockConfig,
        updateConfig: mockUpdateConfig
    };

    it('renders all controls', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

        expect(screen.getByText('live.start_on_launch')).toBeDefined();
        expect(screen.getByText('live.lock_window')).toBeDefined();
        expect(screen.getByText('live.always_on_top')).toBeDefined();
        expect(screen.getByText('live.window_width')).toBeDefined();
        expect(screen.getByText('live.font_size')).toBeDefined();
        expect(screen.getByText('live.font_color')).toBeDefined();
    });

    it('renders width input with correct values and classes', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

        const numberInput = screen.getByDisplayValue('1000');

        expect(numberInput).toBeDefined();
        expect(numberInput.tagName).toBe('INPUT');
        expect(numberInput.getAttribute('type')).toBe('number');
        expect(numberInput.classList.contains('settings-input')).toBe(true);

        // Ensure range slider is removed
        const widthRange = document.querySelector('input[type="range"][value="1000"]');
        expect(widthRange).toBeNull();
    });

    it('calls updateConfig when inputs change', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

        const numberInput = screen.getByDisplayValue('1000');

        fireEvent.change(numberInput, { target: { value: '1200' } });
        expect(mockUpdateConfig).toHaveBeenCalledWith({ captionWindowWidth: 1200 });
    });

    it('renders font size input with correct classes', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

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
        render(<SettingsSubtitleTab {...defaultProps} />);

        const colorInput = screen.getByLabelText('live.font_color');
        expect(colorInput.getAttribute('type')).toBe('color');
        expect(colorInput.getAttribute('value')).toBe('#ffffff');

        const textInput = screen.getByLabelText('live.font_color_hex');
        expect(textInput.getAttribute('type')).toBe('text');
        expect(textInput.getAttribute('value')).toBe('#ffffff');
        expect(textInput.classList.contains('settings-input')).toBe(true);
    });
});
