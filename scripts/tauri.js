import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const tauriBinary = path.resolve(
  repoRoot,
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'tauri.cmd' : 'tauri'
);

const args = process.argv.slice(2);
const command = args[0];

if (command === 'build' || command === 'bundle') {
  const setupFfmpegResult = spawnSync(
    process.execPath,
    [path.resolve(__dirname, 'setup-ffmpeg.js')],
    {
      cwd: repoRoot,
      stdio: 'inherit',
    }
  );

  if (setupFfmpegResult.status !== 0) {
    process.exit(setupFfmpegResult.status ?? 1);
  }
}

const tauriResult = spawnSync(tauriBinary, args, {
  cwd: repoRoot,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

if (tauriResult.error) {
  console.error("Failed to spawn tauri:", tauriResult.error);
  process.exit(1);
}

process.exit(tauriResult.status ?? 1);
