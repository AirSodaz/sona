import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

describe('Tauri security config', () => {
  it('keeps CSP enabled and restricts asset protocol scope to managed app data', () => {
    const configPath = resolve(process.cwd(), 'platforms', 'desktop', 'tauri.conf.json');
    const config = JSON.parse(readFileSync(configPath, 'utf8'));
    const security = config.app.security;

    expect(security.csp).toEqual(expect.any(String));
    expect(security.csp).not.toBe('');
    expect(security.assetProtocol.scope).not.toContain('**');
    expect(security.assetProtocol.scope).toEqual([
      '$APPLOCALDATA/history',
      '$APPLOCALDATA/history/**',
    ]);
  });
});
