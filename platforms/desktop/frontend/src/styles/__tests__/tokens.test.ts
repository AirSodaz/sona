import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { buildTokens } from '../../../scripts/build-tokens.mjs';
import { darkTokens, lightTokens, themeTokens } from '../tokens.ts';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const stylesDir = path.resolve(__dirname, '..');
const tokensJsonPath = path.join(stylesDir, 'tokens.json');
const tokensCssPath = path.join(stylesDir, 'tokens.css');

interface TokenGroup {
  [key: string]: Record<string, string>;
}

interface TokensSchema {
  theme: {
    light: TokenGroup;
    dark: TokenGroup;
  };
  global: TokenGroup;
}

describe('Design System Tokens Dual-Mode Compilation', () => {
  it('exports valid CSS variable references in themeTokens', () => {
    // 1. themeTokens.colors.bgPrimary === 'var(--color-bg-primary)'
    expect(themeTokens.colors.bgPrimary).toBe('var(--color-bg-primary)');

    // 2. themeTokens.colors.danger and error
    expect(themeTokens.colors.danger).toBe('var(--color-danger)');
    expect(themeTokens.colors.error).toBe('var(--color-error)');

    // 3. themeTokens.radius.xs === 'var(--radius-xs)'
    expect(themeTokens.radius.xs).toBe('var(--radius-xs)');

    // 4. themeTokens.button.primaryBg === 'var(--button-primary-bg)'
    expect(themeTokens.button.primaryBg).toBe('var(--button-primary-bg)');

    // 5. themeTokens.apple.controlSurface === 'var(--apple-control-surface)'
    expect(themeTokens.apple.controlSurface).toBe('var(--apple-control-surface)');

    // 6. themeTokens.typography.sans === 'var(--font-sans)'
    expect(themeTokens.typography.sans).toBe('var(--font-sans)');
  });

  it('exports concrete values for light and dark themes', () => {
    // 7. lightTokens and darkTokens bgPrimary
    expect(lightTokens.colors.bgPrimary).toBe('#faf9f7');
    expect(darkTokens.colors.bgPrimary).toBe('#191919');

    // 8. lightTokens and darkTokens button.primaryText
    expect(lightTokens.button.primaryText).toBe('#ffffff');
    expect(darkTokens.button.primaryText).toBe('#191919');
  });

  it('compiles tokens.css containing :root, :root[data-theme="dark"], and all custom property definitions', () => {
    // 9. Verify tokens.css contains selectors and custom properties
    const cssContent = fs.readFileSync(tokensCssPath, 'utf-8');
    expect(cssContent).toContain(':root {');
    expect(cssContent).toContain(':root[data-theme="dark"] {');

    // Check specific custom properties in tokens.css
    expect(cssContent).toContain('--color-bg-primary: #faf9f7;');
    expect(cssContent).toContain('--color-bg-primary: #191919;');
    expect(cssContent).toContain('--color-danger: #e03e3e;');
    expect(cssContent).toContain('--color-error: #e03e3e;');
    expect(cssContent).toContain('--radius-xs: 6px;');
    expect(cssContent).toContain('--button-primary-bg: var(--color-accent-gradient);');
    expect(cssContent).toContain('--apple-control-surface: var(--color-bg-elevated);');
    expect(cssContent).toContain('--font-sans:');

    // Check compatibility aliases
    expect(cssContent).toContain('--bg-primary: var(--color-bg-primary);');
    expect(cssContent).toContain('--bg-secondary: var(--color-bg-secondary);');
    expect(cssContent).toContain('--text-primary: var(--color-text-primary);');
    expect(cssContent).toContain('--border: var(--color-border);');
    expect(cssContent).toContain('--color-primary: var(--color-accent-primary);');
    expect(cssContent).toContain('--color-text-main: var(--color-text-primary);');
  });

  it('maintains parity between tokens.json sections and TypeScript token maps', () => {
    // 10. Verify every section in tokens.json has corresponding entries in themeTokens, lightTokens, and darkTokens
    const rawJson = fs.readFileSync(tokensJsonPath, 'utf-8');
    const tokensJson = JSON.parse(rawJson) as TokensSchema;

    // Check theme sections
    for (const [section, entries] of Object.entries(tokensJson.theme.light)) {
      expect(themeTokens).toHaveProperty(section);
      expect(lightTokens).toHaveProperty(section);
      expect(darkTokens).toHaveProperty(section);

      const themeSection = (themeTokens as Record<string, Record<string, string>>)[section];
      const lightSection = (lightTokens as Record<string, Record<string, string>>)[section];
      const darkSection = (darkTokens as Record<string, Record<string, string>>)[section];

      for (const key of Object.keys(entries)) {
        expect(themeSection).toHaveProperty(key);
        expect(themeSection[key]).toMatch(/^var\(--/);
        expect(lightSection).toHaveProperty(key);
        expect(darkSection).toHaveProperty(key);
      }
    }

    // Check global sections
    for (const [section, entries] of Object.entries(tokensJson.global)) {
      expect(themeTokens).toHaveProperty(section);
      expect(lightTokens).toHaveProperty(section);
      expect(darkTokens).toHaveProperty(section);

      const themeSection = (themeTokens as Record<string, Record<string, string>>)[section];
      const lightSection = (lightTokens as Record<string, Record<string, string>>)[section];
      const darkSection = (darkTokens as Record<string, Record<string, string>>)[section];

      for (const [key, value] of Object.entries(entries)) {
        expect(themeSection).toHaveProperty(key);
        expect(themeSection[key]).toMatch(/^var\(--/);
        expect(lightSection[key]).toBe(value);
        expect(darkSection[key]).toBe(value);
      }
    }
  });

  it('throws an informative error if tokens.json is malformed or contains unknown sections', () => {
    const tempDir = fs.mkdtempSync(path.join(stylesDir, '__tests__', 'tmp-'));
    try {
      const invalidPath = path.join(tempDir, 'invalid.json');
      fs.writeFileSync(invalidPath, JSON.stringify({ invalid: true }));
      expect(() => buildTokens(invalidPath)).toThrow(
        /missing theme\.light, theme\.dark, or global sections/
      );

      const unknownSectionPath = path.join(tempDir, 'unknown.json');
      fs.writeFileSync(
        unknownSectionPath,
        JSON.stringify({
          theme: { light: { unknownFoo: { bar: '1' } }, dark: {} },
          global: {},
        })
      );
      expect(() => buildTokens(unknownSectionPath)).toThrow(/Unknown token section "unknownFoo"/);
    } finally {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
