/**
 * Converts a hex color string (#RGB or #RRGGBB) to an rgba() CSS color string.
 * Falls back to black (rgba(0, 0, 0, opacity)) on invalid inputs.
 */
export function hexToRgba(hex?: string | null, opacity = 1): string {
  let cleaned = (hex || '#000000').replace('#', '').trim();
  if (cleaned.length === 3) {
    cleaned = cleaned
      .split('')
      .map((c) => c + c)
      .join('');
  }
  const clampedOpacity = Math.max(0, Math.min(1, Number.isFinite(opacity) ? opacity : 1));
  if (cleaned.length !== 6 || !/^[0-9A-Fa-f]{6}$/.test(cleaned)) {
    return `rgba(0, 0, 0, ${clampedOpacity})`;
  }
  const r = parseInt(cleaned.slice(0, 2), 16) || 0;
  const g = parseInt(cleaned.slice(2, 4), 16) || 0;
  const b = parseInt(cleaned.slice(4, 6), 16) || 0;
  return `rgba(${r}, ${g}, ${b}, ${clampedOpacity})`;
}
