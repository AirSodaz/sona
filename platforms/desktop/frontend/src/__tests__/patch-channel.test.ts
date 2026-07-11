import { describe, it, expect } from 'vitest';
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import os from 'os';
import { fileURLToPath } from 'url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../..');

describe('patch-channel.js integration', () => {
  it('should correctly patch all configurations for nightly channel', () => {
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'sona-patch-test-'));
    try {
      const scriptsDir = path.join(tempDir, 'platforms', 'desktop', 'scripts');
      const tauriDir = path.join(tempDir, 'platforms', 'desktop');
      const frontendDir = path.join(tauriDir, 'frontend');
      const iconsNightlyDir = path.join(tauriDir, 'icons-nightly');
      const iconsDir = path.join(tauriDir, 'icons');

      fs.mkdirSync(scriptsDir, { recursive: true });
      fs.mkdirSync(frontendDir, { recursive: true });
      fs.mkdirSync(iconsNightlyDir, { recursive: true });
      fs.mkdirSync(iconsDir, { recursive: true });

      // Create a mock icon file in icons-nightly
      const mockIconPath = path.join(iconsNightlyDir, 'icon.png');
      fs.writeFileSync(mockIconPath, 'mock icon content');

      // Create mock frontend package.json
      fs.writeFileSync(path.join(frontendDir, 'package.json'), JSON.stringify({
        name: 'sona-desktop-frontend',
        version: '1.0.0'
      }, null, 2));

      // Create mock workspace Cargo.toml
      fs.writeFileSync(path.join(tempDir, 'Cargo.toml'), `
[workspace.package]
version = "1.0.0"
authors = ["AirSodaz"]
edition = "2024"
      `.trim());

      // Create mock tauri.conf.json
      fs.writeFileSync(path.join(tauriDir, 'tauri.conf.json'), JSON.stringify({
        productName: 'Sona',
        version: '1.0.0',
        identifier: 'com.asoda.sona',
        plugins: {
          updater: {
            endpoints: ['https://example.com/stable/updater.json']
          }
        }
      }, null, 2));

      // Create mock tauri.windows.conf.json
      fs.writeFileSync(path.join(tauriDir, 'tauri.windows.conf.json'), JSON.stringify({
        identifier: 'com.asoda.sona'
      }, null, 2));

      // Copy patch-channel.js script to its desktop platform location.
      const scriptPath = path.join(repoRoot, 'platforms', 'desktop', 'scripts', 'patch-channel.js');
      const destScriptPath = path.join(scriptsDir, 'patch-channel.js');
      fs.copyFileSync(scriptPath, destScriptPath);

      // Execute patching script
      execSync(`node "${destScriptPath}" --channel nightly --version 1.0.0-45`, {
        cwd: tempDir,
        stdio: 'pipe'
      });

      // Assertions
      const patchedPackage = JSON.parse(fs.readFileSync(path.join(frontendDir, 'package.json'), 'utf8'));
      expect(patchedPackage.version).toBe('1.0.0-45');

      const patchedCargo = fs.readFileSync(path.join(tempDir, 'Cargo.toml'), 'utf8');
      expect(patchedCargo).toContain('version = "1.0.0-45"');

      const patchedTauri = JSON.parse(fs.readFileSync(path.join(tauriDir, 'tauri.conf.json'), 'utf8'));
      expect(patchedTauri.version).toBe('1.0.0-45');
      expect(patchedTauri.identifier).toBe('com.asoda.sona.nightly');
      expect(patchedTauri.productName).toBe('Sona-Nightly');
      expect(patchedTauri.plugins.updater.endpoints[0]).toBe('https://github.com/AirSodaz/sona/releases/download/nightly/updater.json');

      const patchedTauriWindows = JSON.parse(fs.readFileSync(path.join(tauriDir, 'tauri.windows.conf.json'), 'utf8'));
      expect(patchedTauriWindows.version).toBe('1.0.0-45');
      expect(patchedTauriWindows.identifier).toBe('com.asoda.sona.nightly');

      // Assert that mock icon file was correctly copied to the mock icons directory
      const copiedIconPath = path.join(iconsDir, 'icon.png');
      expect(fs.existsSync(copiedIconPath)).toBe(true);
      expect(fs.readFileSync(copiedIconPath, 'utf8')).toBe('mock icon content');
    } finally {
      // Cleanup
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
