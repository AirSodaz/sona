import * as esbuild from 'esbuild';
import {copyFileSync, existsSync, mkdirSync, readdirSync} from 'fs';
import {dirname, join} from 'path';
import {fileURLToPath} from 'url';
import {createRequire} from 'module';
import {execSync} from 'child_process';

const require = createRequire(import.meta.url);
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const distDir = join(__dirname, 'dist');
if (!existsSync(distDir)) {
    mkdirSync(distDir);
}

// 1. Bundle JS
console.log('Bundling sherpa-recognizer.js...');
await esbuild.build({
    entryPoints: [join(__dirname, 'sherpa-recognizer.js')],
    bundle: true,
    platform: 'node',
    target: 'node18',
    outfile: join(distDir, 'index.mjs'),
    format: 'esm',
    banner: {
        js: `import { createRequire } from 'module';
import { fileURLToPath as __fileURLToPath } from 'url';
import { dirname as __pathDirname } from 'path';
const require = createRequire(import.meta.url);
const __filename = __fileURLToPath(import.meta.url);
const __dirname = __pathDirname(__filename);
`
    },
    external: [
        '*.node'
    ],
    // loader: { '.node': 'file' }
});

// 2. Copy sherpa-onnx WASM
console.log('Copying sherpa-onnx WASM...');

const wasmFileSrc = join(__dirname, 'node_modules/sherpa-onnx/sherpa-onnx-wasm-nodejs.wasm');
if (existsSync(wasmFileSrc)) {
    copyFileSync(wasmFileSrc, join(distDir, 'sherpa-onnx-wasm-nodejs.wasm'));
    console.log(`Copied sherpa-onnx-wasm-nodejs.wasm to dist/`);
} else {
    console.error(`Could not find sherpa-onnx-wasm-nodejs.wasm at ${wasmFileSrc}`);
    process.exit(1);
}


// 3. Copy ffmpeg
console.log('Copying ffmpeg...');
let ffmpegSrc;
try {
    ffmpegSrc = require('ffmpeg-static');
} catch (e) {
    console.error('Failed to resolve ffmpeg-static:', e);
}

if (ffmpegSrc && existsSync(ffmpegSrc)) {
    const platform = process.platform;
    const destName = platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg';
    copyFileSync(ffmpegSrc, join(distDir, destName));
    console.log(`Copied ffmpeg to dist/${destName}`);
} else {
    console.error('Could not find ffmpeg binary');
    process.exit(1);
}

// 4. Copy 7zip
console.log('Copying 7zip...');
let sevenZipSrc;
try {
    sevenZipSrc = require('7zip-bin').path7za;
} catch (e) {
    console.error('Failed to resolve 7zip-bin:', e);
}

if (sevenZipSrc && existsSync(sevenZipSrc)) {
    const platform = process.platform;
    const destName = platform === 'win32' ? '7za.exe' : '7za';
    copyFileSync(sevenZipSrc, join(distDir, destName));
    console.log(`Copied 7zip to dist/${destName}`);
} else {
    console.warn('Could not find 7zip binary');
}

console.log('Build complete.');
