import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ColorSwatchPicker, getContrastTextColor } from '../ColorSwatchPicker';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}));

describe('ColorSwatchPicker', () => {
  it('renders default preset swatches, divider, and custom color button', () => {
    const onChange = vi.fn();
    const { container } = render(<ColorSwatchPicker value="#6366F1" onChange={onChange} />);

    const swatchesContainer = container.querySelector('.project-color-swatches');
    expect(swatchesContainer).not.toBeNull();

    // The active preset swatch should have the 'active' class
    const indigoSwatch = screen.getByRole('button', { name: '#6366F1' });
    expect(indigoSwatch.classList.contains('active')).toBe(true);

    // Other presets should not have 'active'
    const emeraldSwatch = screen.getByRole('button', { name: '#10B981' });
    expect(emeraldSwatch.classList.contains('active')).toBe(false);

    // Custom button should not be active since #6366F1 is a preset
    const customColorBtn = screen.getByRole('button', { name: 'Custom color' });
    expect(customColorBtn.classList.contains('active')).toBe(false);
  });

  it('marks custom color button active when color is not in presets', () => {
    const onChange = vi.fn();
    render(<ColorSwatchPicker value="#FFFFFF" onChange={onChange} />);

    const customColorBtn = screen.getByRole('button', { name: 'Custom color' });
    expect(customColorBtn.classList.contains('active')).toBe(true);
    expect(customColorBtn.getAttribute('data-tooltip')).toBe('Custom color: #FFFFFF');
  });

  it('calls onChange when clicking a preset swatch', () => {
    const onChange = vi.fn();
    render(<ColorSwatchPicker value="#FFFFFF" onChange={onChange} />);

    const emeraldSwatch = screen.getByRole('button', { name: '#10B981' });
    fireEvent.click(emeraldSwatch);

    expect(onChange).toHaveBeenCalledWith('#10B981');
  });

  it('toggles ColorPicker popover when clicking custom color button', () => {
    const onChange = vi.fn();
    render(<ColorSwatchPicker value="#6366F1" onChange={onChange} />);

    const customColorBtn = screen.getByRole('button', { name: 'Custom color' });
    expect(document.querySelector('.sona-color-picker-popover')).toBeNull();

    // Click to open
    fireEvent.click(customColorBtn);
    expect(document.querySelector('.sona-color-picker-popover')).not.toBeNull();

    // Click again to close
    fireEvent.click(customColorBtn);
    expect(document.querySelector('.sona-color-picker-popover')).toBeNull();
  });

  it('allows changing color inside ColorPicker and calls onChange', () => {
    const onChange = vi.fn();
    render(<ColorSwatchPicker value="#6366F1" onChange={onChange} />);

    const customColorBtn = screen.getByRole('button', { name: 'Custom color' });
    fireEvent.click(customColorBtn);

    const toggleBtn = screen.getByLabelText('Toggle RGB / HEX');
    fireEvent.click(toggleBtn);

    const hexInput = screen.getByDisplayValue('#6366F1');
    fireEvent.change(hexInput, { target: { value: '#123456' } });

    expect(onChange).toHaveBeenCalledWith('#123456');
  });

  it('supports custom presets prop', () => {
    const onChange = vi.fn();
    const customPresets = ['#FF0000', '#00FF00', '#0000FF'];
    render(<ColorSwatchPicker value="#FF0000" presets={customPresets} onChange={onChange} />);

    expect(screen.getByRole('button', { name: '#FF0000' })).toBeDefined();
    expect(screen.getByRole('button', { name: '#00FF00' })).toBeDefined();
    expect(screen.getByRole('button', { name: '#0000FF' })).toBeDefined();
    expect(screen.queryByRole('button', { name: '#10B981' })).toBeNull();
  });

  describe('getContrastTextColor', () => {
    it('returns dark text for light background and light text for dark background', () => {
      expect(getContrastTextColor('#FFFFFF')).toBe('#0f172a');
      expect(getContrastTextColor('#000000')).toBe('#ffffff');
      expect(getContrastTextColor('#6366F1')).toBe('#ffffff');
      expect(getContrastTextColor('#FEF08A')).toBe('#0f172a');
      expect(getContrastTextColor('')).toBe('#ffffff');
    });
  });
});
