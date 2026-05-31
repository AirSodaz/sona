import { describe, it, expect } from 'vitest';
import { LANGUAGE_OPTIONS } from '../languages';

describe('LANGUAGE_OPTIONS', () => {
  it('should be a non-empty array with code and englishName', () => {
    expect(LANGUAGE_OPTIONS).toBeDefined();
    expect(Array.isArray(LANGUAGE_OPTIONS)).toBe(true);
    expect(LANGUAGE_OPTIONS.length).toBeGreaterThan(100);

    const first = LANGUAGE_OPTIONS[0];
    expect(first).toHaveProperty('code');
    expect(first).toHaveProperty('englishName');
    expect(typeof first.code).toBe('string');
    expect(typeof first.englishName).toBe('string');
  });

  it('should contain key translation languages', () => {
    const codes = LANGUAGE_OPTIONS.map(opt => opt.code);
    expect(codes).toContain('zh');
    expect(codes).toContain('en');
    expect(codes).toContain('ja');
    expect(codes).toContain('ko');
    expect(codes).toContain('es');
    expect(codes).toContain('fr');
    expect(codes).toContain('de');
  });
});
