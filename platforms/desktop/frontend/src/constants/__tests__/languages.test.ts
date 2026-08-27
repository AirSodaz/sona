import { describe, it, expect } from 'vitest';
import { LANGUAGE_OPTIONS } from '../languages';

describe('LANGUAGE_OPTIONS', () => {
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
