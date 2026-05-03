import { describe, expect, it } from 'vitest';
import * as exportFormats from '../exportFormats';

describe('exportFormats', () => {
  describe('formatDisplayTime', () => {
    it('formats seconds for playback UI labels', () => {
      expect(exportFormats.formatDisplayTime(0)).toBe('00:00.0');
      expect(exportFormats.formatDisplayTime(65.49)).toBe('01:05.4');
      expect(exportFormats.formatDisplayTime(3599.99)).toBe('59:59.9');
    });
  });

  describe('getFileExtension', () => {
    it.each([
      ['srt', '.srt'],
      ['vtt', '.vtt'],
      ['json', '.json'],
      ['txt', '.txt'],
    ] as const)('returns %s extension', (format, expected) => {
      expect(exportFormats.getFileExtension(format)).toBe(expected);
    });
  });

  describe('getMimeType', () => {
    it.each([
      ['srt', 'text/plain'],
      ['vtt', 'text/plain'],
      ['txt', 'text/plain'],
      ['json', 'application/json'],
    ] as const)('returns %s MIME type', (format, expected) => {
      expect(exportFormats.getMimeType(format)).toBe(expected);
    });
  });

  it('does not expose runtime transcript body formatters from the frontend helper module', () => {
    expect(exportFormats).not.toHaveProperty('exportSegments');
    expect(exportFormats).not.toHaveProperty('toSRT');
    expect(exportFormats).not.toHaveProperty('toVTT');
    expect(exportFormats).not.toHaveProperty('toTXT');
    expect(exportFormats).not.toHaveProperty('toJSON');
  });
});
