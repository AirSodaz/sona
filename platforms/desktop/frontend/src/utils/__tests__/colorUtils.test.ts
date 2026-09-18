import { describe, expect, it } from 'vitest';
import { hexToRgba } from '../colorUtils';

describe('hexToRgba', () => {
  it('converts 6-digit hex to rgba with opacity', () => {
    expect(hexToRgba('#ffffff', 0.6)).toBe('rgba(255, 255, 255, 0.6)');
    expect(hexToRgba('#000000', 0.5)).toBe('rgba(0, 0, 0, 0.5)');
    expect(hexToRgba('#6366F1', 1)).toBe('rgba(99, 102, 241, 1)');
  });

  it('converts 3-digit shorthand hex', () => {
    expect(hexToRgba('#fff', 0.8)).toBe('rgba(255, 255, 255, 0.8)');
    expect(hexToRgba('#000', 0.2)).toBe('rgba(0, 0, 0, 0.2)');
  });

  it('clamps opacity to [0, 1]', () => {
    expect(hexToRgba('#ffffff', -0.5)).toBe('rgba(255, 255, 255, 0)');
    expect(hexToRgba('#ffffff', 1.5)).toBe('rgba(255, 255, 255, 1)');
  });

  it('falls back gracefully on invalid or missing inputs', () => {
    expect(hexToRgba(undefined, 0.6)).toBe('rgba(0, 0, 0, 0.6)');
    expect(hexToRgba(null, 0.6)).toBe('rgba(0, 0, 0, 0.6)');
    expect(hexToRgba('invalid', 0.6)).toBe('rgba(0, 0, 0, 0.6)');
  });
});
