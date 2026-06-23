// @vitest-environment node
import { describe, it, expect } from 'vitest';
import { mkdtempSync, writeFileSync, readFileSync, mkdirSync, rmSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { getPlatformKey, generateUpdaterJson } from './generate-nightly-updater-json.js';

describe('getPlatformKey', () => {
  it('returns null for .msi', () => {
    expect(getPlatformKey('Sona_0.8.0_x64_en-US.msi')).toBeNull();
  });

  it('detects windows-x86_64 from setup .exe', () => {
    expect(getPlatformKey('Sona_0.8.0_x64-setup.exe')).toBe('windows-x86_64');
  });

  it('returns null for non-setup .exe', () => {
    expect(getPlatformKey('Sona_0.8.0_x64.exe')).toBeNull();
  });

  it('returns null for arm64 .msi', () => {
    expect(getPlatformKey('Sona_0.8.0_arm64_en-US.msi')).toBeNull();
  });

  it('returns null for .dmg', () => {
    expect(getPlatformKey('Sona_aarch64.dmg')).toBeNull();
  });

  it('detects darwin-x86_64 from _x64.app.tar.gz', () => {
    expect(getPlatformKey('Sona_x64.app.tar.gz')).toBe('darwin-x86_64');
  });

  it('returns null for universal.dmg', () => {
    expect(getPlatformKey('Sona_universal.dmg')).toBeNull();
  });

  it('returns null for universal.app.tar.gz', () => {
    expect(getPlatformKey('Sona_universal.app.tar.gz')).toBeNull();
  });

  it('detects linux-x86_64 from .AppImage', () => {
    expect(getPlatformKey('Sona_0.8.0_amd64.AppImage')).toBe('linux-x86_64');
  });

  it('returns null for .deb', () => {
    expect(getPlatformKey('sona_0.8.0_amd64.deb')).toBeNull();
  });

  it('returns null for .rpm', () => {
    expect(getPlatformKey('sona-0.8.0-1.x86_64.rpm')).toBeNull();
  });

  it('returns null for unknown extensions', () => {
    expect(getPlatformKey('random-file.txt')).toBeNull();
  });

  it('detects darwin-x86_64 from artifact zip name', () => {
    expect(getPlatformKey('nightly-x86_64-apple-darwin-sha.zip')).toBe('darwin-x86_64');
  });

  it('detects darwin-aarch64 from artifact zip name', () => {
    expect(getPlatformKey('nightly-aarch64-apple-darwin-sha.zip')).toBe('darwin-aarch64');
  });
});

describe('generateUpdaterJson', () => {
  let tmpDir;

  function createFixture(files) {
    tmpDir = mkdtempSync(join(tmpdir(), 'sona-updater-test-'));
    for (const [name, content] of Object.entries(files)) {
      const filePath = join(tmpDir, name);
      if (content !== null) {
        writeFileSync(filePath, content, 'utf-8');
      } else {
        writeFileSync(filePath, '', 'utf-8');
      }
    }
    return tmpDir;
  }

  function cleanup() {
    if (tmpDir) {
      rmSync(tmpDir, { recursive: true, force: true });
      tmpDir = null;
    }
  }

  it('generates updater JSON with all platforms', () => {
    const dir = createFixture({
      'Sona_aarch64.app.tar.gz': 'tar-file',
      'Sona_aarch64.app.tar.gz.sig': 'mac-sig-aarch64',
      'Sona_x64.app.tar.gz': 'tar-x64-file',
      'Sona_x64.app.tar.gz.sig': 'mac-sig-x64',
      'Sona_0.8.0_x64-setup.exe': 'exe-file',
      'Sona_0.8.0_x64-setup.exe.sig': 'win-sig-x64',
      'Sona_0.8.0_arm64-setup.exe': 'exe-arm64-file',
      'Sona_0.8.0_arm64-setup.exe.sig': 'win-sig-arm64',
      'Sona_0.8.0_amd64.AppImage': 'appimage-file',
      'Sona_0.8.0_amd64.AppImage.sig': 'linux-sig',
      // These should be ignored by the generator
      'Sona_aarch64.dmg': 'dmg-file',
      'Sona_aarch64.dmg.sig': 'mac-sig-aarch64',
      'Sona_0.8.0_x64_en-US.msi': 'msi-file',
      'sona_0.8.0_amd64.deb': 'deb-file',
    });

    const result = generateUpdaterJson(dir);

    expect(result.version).toBeDefined();
    expect(result.platforms['darwin-aarch64']).toBeDefined();
    expect(result.platforms['darwin-aarch64'].signature).toBe('mac-sig-aarch64');
    expect(result.platforms['darwin-x86_64']).toBeDefined();
    expect(result.platforms['darwin-x86_64'].signature).toBe('mac-sig-x64');
    expect(result.platforms['windows-x86_64']).toBeDefined();
    expect(result.platforms['windows-x86_64'].signature).toBe('win-sig-x64');
    expect(result.platforms['windows-aarch64']).toBeDefined();
    expect(result.platforms['windows-aarch64'].signature).toBe('win-sig-arm64');
    expect(result.platforms['linux-x86_64']).toBeDefined();
    expect(result.platforms['linux-x86_64'].signature).toBe('linux-sig');

    cleanup();
  });

  it('throws on missing signature', () => {
    const dir = createFixture({
      'Sona_aarch64.app.tar.gz': 'tar-file',
    });

    expect(() => generateUpdaterJson(dir)).toThrow('No signature found');

    cleanup();
  });

  it('throws on duplicate platform key', () => {
    const dir = createFixture({
      'Sona_aarch64.app.tar.gz': 'tar-file',
      'Sona_aarch64.app.tar.gz.sig': 'sig',
      'Sona_arm64.app.tar.gz': 'tar-file-2',
      'Sona_arm64.app.tar.gz.sig': 'sig-2',
    });

    expect(() => generateUpdaterJson(dir)).toThrow('Duplicate platform key');

    cleanup();
  });
});
