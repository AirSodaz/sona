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

// Map Node process.platform and process.arch to Tauri target triples
let targetTriple = '';
const platform = process.platform;
const arch = process.arch;

if (platform === 'darwin') {
  targetTriple = arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
} else if (platform === 'win32') {
  targetTriple = arch === 'arm64' ? 'aarch64-pc-windows-msvc' : 'x86_64-pc-windows-msvc';
} else if (platform === 'linux') {
  targetTriple = arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : 'x86_64-unknown-linux-gnu';
} else {
  console.error(`Unsupported platform/architecture: ${platform}/${arch}`);
  process.exit(1);
}

const ext = platform === 'win32' ? '.exe' : '';
const targetFilename = `ffmpeg-${targetTriple}${ext}`;
const targetPath = path.join(binariesDir, targetFilename);

if (fs.existsSync(targetPath)) {
  console.log(`[ffmpeg-setup] Binary already exists at ${targetPath}. Skipping download/copy.`);
} else {
  console.log(`[ffmpeg-setup] Copying ffmpeg from ${ffmpegStatic} to ${targetPath}`);
  fs.copyFileSync(ffmpegStatic, targetPath);

  if (platform !== 'win32') {
    fs.chmodSync(targetPath, 0o755);
  }
}
