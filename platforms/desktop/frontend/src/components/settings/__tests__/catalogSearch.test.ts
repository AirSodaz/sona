import { describe, expect, it } from 'vitest';
import { isPrimarilySectionAliasQuery, resolveCatalogSectionAliases } from '../catalogSearch';

describe('catalogSearch aliases', () => {
  it('maps vad / silero queries to the VAD section', () => {
    expect(resolveCatalogSectionAliases('vad')).toEqual(['vad']);
    expect(resolveCatalogSectionAliases('silero')).toEqual(['vad']);
    expect(resolveCatalogSectionAliases('VAD')).toEqual(['vad']);
  });

  it('maps punctuation and speaker aliases', () => {
    expect(resolveCatalogSectionAliases('punctuation')).toEqual(['punctuation']);
    expect(resolveCatalogSectionAliases('speaker')).toEqual([
      'speaker-segmentation',
      'speaker-embedding',
    ]);
  });

  it('does not treat single-letter noise as an alias', () => {
    expect(resolveCatalogSectionAliases('a')).toEqual([]);
    expect(resolveCatalogSectionAliases('')).toEqual([]);
  });

  it('detects pure section-alias queries', () => {
    expect(isPrimarilySectionAliasQuery('vad')).toBe(true);
    expect(isPrimarilySectionAliasQuery('silero')).toBe(true);
    expect(isPrimarilySectionAliasQuery('silero onnx')).toBe(false);
  });
});
