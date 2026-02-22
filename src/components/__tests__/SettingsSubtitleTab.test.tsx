import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsSubtitleTab } from '../settings/SettingsSubtitleTab';

// Mock translation
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

describe('SettingsSubtitleTab', () => {
    const defaultProps = {
        lockWindow: false,
        setLockWindow: vi.fn(),
        alwaysOnTop: false,
        setAlwaysOnTop: vi.fn(),
        startOnLaunch: false,
        setStartOnLaunch: vi.fn(),
        captionWindowWidth: 1000,
        setCaptionWindowWidth: vi.fn(),
        captionFontSize: 24,
        setCaptionFontSize: vi.fn(),
        captionFontColor: '#ffffff',
        setCaptionFontColor: vi.fn(),
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

    it('calls setCaptionWindowWidth when inputs change', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

        const numberInput = screen.getByDisplayValue('1000');

        fireEvent.change(numberInput, { target: { value: '1200' } });
        expect(defaultProps.setCaptionWindowWidth).toHaveBeenCalledWith(1200);
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
