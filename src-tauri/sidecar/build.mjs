import * as esbuild from 'esbuild';
import { copyFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

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
        js: `import { createRequire } from 'module';const require = createRequire(import.meta.url);`
    },
    external: [
        'sherpa-onnx.node'
    ],
    loader: { '.node': 'file' }
});

// 2. Copy sherpa-onnx.node
console.log('Copying native bindings...');

const platform = process.platform;
const arch = process.arch;

let platformName = platform;
if (platform === 'win32') platformName = 'win';
// linux is linux, darwin is darwin

let archName = arch;
// x64 is x64, arm64 is arm64

const packageName = `sherpa-onnx-${platformName}-${archName}`;
const nodeFileSrc = join(__dirname, 'node_modules', packageName, 'sherpa-onnx.node');

console.log(`Looking for binding at: ${nodeFileSrc}`);

if (existsSync(nodeFileSrc)) {
    copyFileSync(nodeFileSrc, join(distDir, 'sherpa-onnx.node'));
    console.log(`Copied ${packageName}/sherpa-onnx.node to dist/`);
} else {
    console.error(`Could not find sherpa-onnx.node at ${nodeFileSrc}`);
    // List available modules to debug
    try {
        const fs = require('fs');
        const modulesDir = join(__dirname, 'node_modules');
        const dirs = fs.readdirSync(modulesDir).filter(d => d.startsWith('sherpa-onnx-'));
        console.log('Available sherpa-onnx modules:', dirs);
    } catch (e) {}
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
    const destName = platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg';
    copyFileSync(ffmpegSrc, join(distDir, destName));
    console.log(`Copied ffmpeg to dist/${destName}`);
} else {
    console.error('Could not find ffmpeg binary');
    process.exit(1);
}

console.log('Build complete.');
