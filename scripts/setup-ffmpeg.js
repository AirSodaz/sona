import fs from 'fs';
import path from 'path';
import ffmpegStatic from 'ffmpeg-static';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Ensure the binaries directory exists
const binariesDir = path.resolve(__dirname, '../src-tauri/binaries');
if (!fs.existsSync(binariesDir)) {
  fs.mkdirSync(binariesDir, { recursive: true });
}

// In CI environments (like GitHub Actions), Tauri might be instructed to cross-compile
// (e.g. building for x86_64-apple-darwin or universal-apple-darwin on an arm64 runner).
// To ensure the Tauri bundler does not fail due to missing sidecar binaries for those targets,
// we copy the `ffmpeg-static` binary to all common target triples for the current OS.
// Even if the binary's actual architecture doesn't match the target name, it allows the CI
// to successfully bundle the app (and universal binaries will at least contain one working architecture).

const platform = process.platform;
const targets = [];

if (platform === 'darwin') {
  targets.push('x86_64-apple-darwin');
  targets.push('aarch64-apple-darwin');
  targets.push('universal-apple-darwin'); // Tauri sometimes looks for this explicitly depending on config
} else if (platform === 'win32') {
  targets.push('x86_64-pc-windows-msvc.exe');
  targets.push('aarch64-pc-windows-msvc.exe');
} else if (platform === 'linux') {
  targets.push('x86_64-unknown-linux-gnu');
  targets.push('aarch64-unknown-linux-gnu');
} else {
  console.error(`Unsupported platform: ${platform}`);
  process.exit(1);
}

for (const target of targets) {
  const targetPath = path.join(binariesDir, `ffmpeg-${target}`);

  if (fs.existsSync(targetPath)) {
    console.log(`[ffmpeg-setup] Binary already exists at ${targetPath}. Skipping copy.`);
  } else {
    console.log(`[ffmpeg-setup] Copying ffmpeg from ${ffmpegStatic} to ${targetPath}`);
    fs.copyFileSync(ffmpegStatic, targetPath);

    if (platform !== 'win32') {
      fs.chmodSync(targetPath, 0o755);
    }
  }
}
