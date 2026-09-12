import { useEffect } from 'react';

const SPIN_ATTRIBUTE = 'data-spin';
type StepperHalf = 'up' | 'down';

interface StepperGeometry {
  stepperWidth: number;
  rightInset: number;
}

/** Mirrors the pointer position to CSS because Chromium exposes one stepper pseudo-element. */
export function useNumberStepperHoverEffect(): void {
  useEffect(() => {
    const geometryCache = new WeakMap<HTMLInputElement, StepperGeometry>();
    let hovered: HTMLInputElement | null = null;

    const getGeometry = (input: HTMLInputElement): StepperGeometry => {
      const cached = geometryCache.get(input);
      if (cached) return cached;

      const styles = getComputedStyle(input);
      const geometry = {
        stepperWidth: Number.parseFloat(styles.getPropertyValue('--number-stepper-width')) || 0,
        rightInset: Number.parseFloat(styles.paddingRight) || 0,
      };
      geometryCache.set(input, geometry);
      return geometry;
    };

    const release = () => {
      hovered?.removeAttribute(SPIN_ATTRIBUTE);
      hovered = null;
    };

    const handlePointerMove = (event: PointerEvent) => {
      const input = event.target;
      if (
        !(input instanceof HTMLInputElement) ||
        input.type !== 'number' ||
        input.disabled ||
        input.readOnly
      ) {
        release();
        return;
      }

      if (hovered !== input) {
        release();
        hovered = input;
      }

      const { stepperWidth, rightInset } = getGeometry(input);
      const { right, top, height } = input.getBoundingClientRect();
      const overStepper = event.clientX >= right - rightInset - stepperWidth;
      const half: StepperHalf | null = overStepper
        ? event.clientY < top + height / 2
          ? 'up'
          : 'down'
        : null;

      if (half) input.setAttribute(SPIN_ATTRIBUTE, half);
      else input.removeAttribute(SPIN_ATTRIBUTE);
    };

    document.addEventListener('pointermove', handlePointerMove, true);
    document.addEventListener('pointerleave', release);
    window.addEventListener('blur', release);

    return () => {
      document.removeEventListener('pointermove', handlePointerMove, true);
      document.removeEventListener('pointerleave', release);
      window.removeEventListener('blur', release);
      release();
    };
  }, []);
}
