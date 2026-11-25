import type React from 'react';
import { useCallback, useLayoutEffect, useRef } from 'react';
import './Slider.css';

export type SliderSize = 'sm' | 'md';

export interface SliderProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type' | 'size'> {
  /** Visual size variant. `sm` fits compact toolbars/players, `md` fits settings rows. */
  size?: SliderSize;
  /** Optional ref forwarded to the underlying `<input type="range">` element. */
  ref?: React.Ref<HTMLInputElement>;
}

/**
 * Computes the normalized fill percentage (0-100) for a range slider.
 * Guards against non-finite inputs and degenerate ranges (`max <= min`).
 */
export function computeSliderProgress(
  value: number | string | readonly string[] | undefined,
  min: number | string | undefined,
  max: number | string | undefined
): number {
  const numericMin = Number(min ?? 0);
  const numericMax = Number(max ?? 100);
  const safeMin = Number.isFinite(numericMin) ? numericMin : 0;
  const safeMax = Number.isFinite(numericMax) ? numericMax : 100;

  if (safeMax <= safeMin) {
    return 0;
  }

  const rawValue = Array.isArray(value) ? value[0] : value;
  const numericVal = Number(rawValue ?? safeMin);
  if (!Number.isFinite(numericVal)) {
    return 0;
  }

  const ratio = ((numericVal - safeMin) / (safeMax - safeMin)) * 100;
  return Math.max(0, Math.min(100, ratio));
}

/**
 * Synchronizes `--slider-progress` directly on an `<input type="range">` DOM node.
 * Useful for store-driven imperative sliders (e.g. audio playback seek) that bypass React renders.
 */
export function syncSliderProgress(
  input: HTMLInputElement | null,
  minOverride?: number | string,
  maxOverride?: number | string
): void {
  if (!input) return;
  const progress = computeSliderProgress(
    input.value,
    minOverride ?? (input.min || 0),
    maxOverride ?? (input.max || 100)
  );
  input.style.setProperty('--slider-progress', `${progress}%`);
}

/**
 * Unified native `<input type="range">` primitive matching Sona's Warm Minimalism design language.
 */
export function Slider({
  size = 'md',
  className,
  style,
  min = 0,
  max = 100,
  value,
  defaultValue,
  onInput,
  ref,
  ...rest
}: SliderProps): React.JSX.Element {
  const localRef = useRef<HTMLInputElement | null>(null);

  const setRef = useCallback(
    (node: HTMLInputElement | null) => {
      localRef.current = node;
      if (typeof ref === 'function') {
        ref(node);
      } else if (ref) {
        ref.current = node;
      }
    },
    [ref]
  );

  useLayoutEffect(() => {
    if (value === undefined && localRef.current) {
      syncSliderProgress(localRef.current, min, max);
    }
  }, [value, min, max]);

  const handleInput = (event: React.InputEvent<HTMLInputElement>) => {
    syncSliderProgress(event.currentTarget);
    onInput?.(event);
  };

  const controlledStyle =
    value !== undefined
      ? ({
          '--slider-progress': `${computeSliderProgress(value, min, max)}%`,
        } as React.CSSProperties)
      : undefined;
  const classes = ['sona-slider', `sona-slider--${size}`, className ?? '']
    .filter(Boolean)
    .join(' ');

  return (
    <input
      {...rest}
      ref={setRef}
      type="range"
      min={min}
      max={max}
      value={value}
      defaultValue={defaultValue}
      onInput={handleInput}
      className={classes}
      style={{
        ...controlledStyle,
        ...style,
      }}
    />
  );
}
