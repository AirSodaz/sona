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

    it('renders width inputs with correct values and classes', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

        const inputs = screen.getAllByDisplayValue('1000');
        const numberInput = inputs.find(input => input.getAttribute('type') === 'number');

        expect(numberInput).toBeDefined();
        expect(numberInput?.classList.contains('settings-input')).toBe(true);

        const rangeInputs = screen.getAllByRole('slider');
        const widthRange = rangeInputs.find(input => input.getAttribute('value') === '1000');
        expect(widthRange).toBeDefined();
        expect(widthRange?.classList.contains('audio-slider')).toBe(true);
    });

    it('calls setCaptionWindowWidth when inputs change', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

        const inputs = screen.getAllByDisplayValue('1000');
        const numberInput = inputs.find(input => input.getAttribute('type') === 'number');

        if (!numberInput) throw new Error('Number input not found');

        fireEvent.change(numberInput, { target: { value: '1200' } });
        expect(defaultProps.setCaptionWindowWidth).toHaveBeenCalledWith(1200);
    });

    it('renders font size inputs with correct classes', () => {
        render(<SettingsSubtitleTab {...defaultProps} />);

        const inputs = screen.getAllByDisplayValue('24');
        const numberInput = inputs.find(input => input.getAttribute('type') === 'number');

        expect(numberInput).toBeDefined();
        expect(numberInput?.classList.contains('settings-input')).toBe(true);

        const rangeInputs = screen.getAllByRole('slider');
        const fontRange = rangeInputs.find(input => input.getAttribute('value') === '24');
        expect(fontRange?.classList.contains('audio-slider')).toBe(true);
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
