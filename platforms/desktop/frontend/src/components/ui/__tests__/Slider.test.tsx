import { fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { computeSliderProgress, Slider, syncSliderProgress } from '../Slider';

describe('Slider', () => {
  it('computes progress percentage safely across edge cases', () => {
    expect(computeSliderProgress(50, 0, 100)).toBe(50);
    expect(computeSliderProgress(1, 0, 2)).toBe(50);
    expect(computeSliderProgress(150, 0, 100)).toBe(100);
    expect(computeSliderProgress(-10, 0, 100)).toBe(0);
    expect(computeSliderProgress(10, 0, 0)).toBe(0);
    expect(computeSliderProgress(10, 50, 10)).toBe(0);
    expect(computeSliderProgress('invalid', 0, 100)).toBe(0);
  });

  it('renders a native range input with default md size and progress CSS variable', () => {
    render(<Slider aria-label="Opacity" min={0} max={100} value={40} onChange={() => {}} />);

    const slider = screen.getByRole('slider', { name: 'Opacity' }) as HTMLInputElement;
    expect(slider.type).toBe('range');
    expect(slider.classList.contains('sona-slider')).toBe(true);
    expect(slider.classList.contains('sona-slider--md')).toBe(true);
    expect(slider.style.getPropertyValue('--slider-progress')).toBe('40%');
  });

  it('supports sm size, custom className, merged style, and disabled state', () => {
    render(
      <Slider
        aria-label="Volume"
        size="sm"
        min={0}
        max={1}
        step={0.01}
        value={0.25}
        disabled
        className="custom-slider"
        style={{ width: '80px' }}
        onChange={() => {}}
      />
    );

    const slider = screen.getByRole('slider', { name: 'Volume' }) as HTMLInputElement;
    expect(slider.classList.contains('sona-slider--sm')).toBe(true);
    expect(slider.classList.contains('custom-slider')).toBe(true);
    expect(slider.disabled).toBe(true);
    expect(slider.style.width).toBe('80px');
    expect(slider.style.getPropertyValue('--slider-progress')).toBe('25%');
  });

  it('forwards ref and updates --slider-progress via onInput and syncSliderProgress', () => {
    const ref = createRef<HTMLInputElement>();
    const handleInput = vi.fn();

    render(
      <Slider
        ref={ref}
        aria-label="Seek"
        size="sm"
        min={0}
        max={200}
        defaultValue={0}
        onInput={handleInput}
      />
    );

    expect(ref.current).not.toBeNull();
    expect(ref.current?.style.getPropertyValue('--slider-progress')).toBe('0%');

    if (ref.current) {
      fireEvent.input(ref.current, { target: { value: '50' } });
      expect(handleInput).toHaveBeenCalledTimes(1);
      expect(ref.current.style.getPropertyValue('--slider-progress')).toBe('25%');

      ref.current.value = '150';
      syncSliderProgress(ref.current);
      expect(ref.current.style.getPropertyValue('--slider-progress')).toBe('75%');
    }
  });
});
