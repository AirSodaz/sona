import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IconPicker } from '../IconPicker';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}));

describe('IconPicker', () => {
  it('labels the custom emoji input with the same text as its placeholder', () => {
    render(<IconPicker icon="" onChange={vi.fn()} />);

    fireEvent.click(screen.getByRole('button'));

    const customEmojiInput = screen.getByRole('textbox', { name: 'Custom emoji' });

    expect(customEmojiInput.getAttribute('placeholder')).toBe('Custom emoji');
  });

  it('renders custom color rounded rectangle and custom tooltips without native titles', () => {
    const onColorChange = vi.fn();
    const onChange = vi.fn();
    render(
      <IconPicker
        icon="system:folder"
        color="#5B82E8"
        onChange={onChange}
        onColorChange={onColorChange}
      />
    );

    // Open picker
    const triggerBtn = screen.getByRole('button', { name: 'Choose icon' }) as HTMLButtonElement;
    expect(triggerBtn).toBeDefined();
    expect(triggerBtn.getAttribute('title')).toBeNull();
    fireEvent.focus(triggerBtn);
    expect(screen.getByRole('tooltip').textContent).toBe('Choose icon');
    fireEvent.blur(triggerBtn);
    fireEvent.click(triggerBtn);

    // Check custom color rounded rectangle
    const customColorBtn = screen.getByRole('button', { name: 'Custom color' });
    expect(customColorBtn).toBeDefined();
    expect(customColorBtn.className).toContain('project-custom-color-swatch');
    expect(customColorBtn.getAttribute('title')).toBeNull();
    fireEvent.focus(customColorBtn);
    expect(screen.getByRole('tooltip').textContent).toBe('Custom color');
    fireEvent.blur(customColorBtn);

    // Clicking custom color button opens custom ColorPicker and allows changing color
    fireEvent.click(customColorBtn);
    const hexToggleBtn = screen.getByLabelText('Toggle RGB / HEX');
    fireEvent.click(hexToggleBtn);
    const hexInput = screen.getByDisplayValue('#5B82E8');
    fireEvent.change(hexInput, { target: { value: '#123456' } });
    expect(onColorChange).toHaveBeenCalledWith('#123456');
    // Check system icons have custom tooltips without native title
    const micBtn = screen.getByRole('button', { name: 'Microphone' });
    expect(micBtn.getAttribute('title')).toBeNull();
    fireEvent.focus(micBtn);
    expect(screen.getByRole('tooltip').textContent).toBe('Microphone');
    fireEvent.blur(micBtn);

    // Check preset colors have custom tooltips without native title
    const presetBtn = screen.getByRole('button', { name: '#4EC39D' });
    expect(presetBtn.getAttribute('title')).toBeNull();
    fireEvent.focus(presetBtn);
    expect(screen.getByRole('tooltip').textContent).toBe('#4EC39D');
  });
  it('does not close IconPicker or ColorPicker when clicking pipette, inputs, or toggle button in ColorPicker', () => {
    const onColorChange = vi.fn();
    const onChange = vi.fn();
    render(
      <IconPicker
        icon="system:folder"
        color="#5B82E8"
        onChange={onChange}
        onColorChange={onColorChange}
      />
    );

    // Open IconPicker
    const triggerBtn = screen.getByRole('button', { name: 'Choose icon' }) as HTMLButtonElement;
    fireEvent.click(triggerBtn);

    // Open ColorPicker
    const customColorBtn = screen.getByRole('button', { name: 'Custom color' });
    fireEvent.click(customColorBtn);

    // Verify ColorPicker popover is open
    expect(document.querySelector('.sona-color-picker-popover')).toBeDefined();

    // 1. Click Pipette button -> should NOT close ColorPicker or IconPicker
    const pipetteBtn = screen.getByLabelText('Pick color');
    fireEvent.mouseDown(pipetteBtn);
    fireEvent.click(pipetteBtn);
    expect(document.querySelector('.sona-color-picker-popover')).not.toBeNull();
    expect(document.querySelector('.icon-picker-popover')).not.toBeNull();

    // 2. Click R input -> should NOT close ColorPicker or IconPicker
    const rInput = screen.getByDisplayValue('91');
    fireEvent.mouseDown(rInput);
    fireEvent.click(rInput);
    expect(document.querySelector('.sona-color-picker-popover')).not.toBeNull();
    expect(document.querySelector('.icon-picker-popover')).not.toBeNull();

    // 3. Click Toggle RGB / HEX button -> should NOT close ColorPicker or IconPicker
    const toggleBtn = screen.getByLabelText('Toggle RGB / HEX');
    fireEvent.mouseDown(toggleBtn);
    fireEvent.click(toggleBtn);
    expect(document.querySelector('.sona-color-picker-popover')).not.toBeNull();
    expect(document.querySelector('.icon-picker-popover')).not.toBeNull();

    // 4. Click HEX text input -> should NOT close ColorPicker or IconPicker
    const hexInput = screen.getByDisplayValue('#5B82E8');
    fireEvent.mouseDown(hexInput);
    fireEvent.click(hexInput);
    expect(document.querySelector('.sona-color-picker-popover')).not.toBeNull();
    expect(document.querySelector('.icon-picker-popover')).not.toBeNull();
  });
  it('selects system icons on click', () => {
    const onChange = vi.fn();
    render(<IconPicker icon="system:folder" onChange={onChange} />);

    // Open IconPicker
    const triggerBtn = screen.getByRole('button', { name: 'Choose icon' }) as HTMLButtonElement;
    fireEvent.click(triggerBtn);

    // Click on Microphone system icon
    const micBtn = screen.getByRole('button', { name: 'Microphone' });
    fireEvent.click(micBtn);

    expect(onChange).toHaveBeenCalledWith('system:mic');
    expect(document.querySelector('.icon-picker-popover')).toBeNull();
  });
});
