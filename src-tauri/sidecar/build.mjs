import * as esbuild from 'esbuild';
import { copyFileSync, mkdirSync, existsSync, readdirSync } from 'fs';
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
        js: `import { createRequire } from 'module';
import { fileURLToPath as __fileURLToPath } from 'url';
import { dirname as __pathDirname } from 'path';
const require = createRequire(import.meta.url);
const __filename = __fileURLToPath(import.meta.url);
const __dirname = __pathDirname(__filename);
`
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
const bindingDir = join(__dirname, 'node_modules', packageName);
const nodeFileSrc = join(bindingDir, 'sherpa-onnx.node');

console.log(`Looking for binding at: ${nodeFileSrc}`);

if (existsSync(nodeFileSrc)) {
    const files = readdirSync(bindingDir);
    for (const file of files) {
        if (file.endsWith('.node') || file.endsWith('.dll') || file.endsWith('.dylib') || file.endsWith('.so')) {
            copyFileSync(join(bindingDir, file), join(distDir, file));
            console.log(`Copied ${file} to dist/`);
        }
    }
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

// 4. Copy sherpa-ncnn assets
console.log('Copying sherpa-ncnn assets...');
const ncnnDir = join(__dirname, 'node_modules', 'sherpa-ncnn');
if (existsSync(ncnnDir)) {
    const files = readdirSync(ncnnDir);
    for (const file of files) {
        if (file.endsWith('.wasm') || (file.endsWith('.js') && file !== 'index.js')) {
             // We copy helper JS files (like the wasm loader) to be safe,
             // though if bundled, they might be inlined.
             // But usually WASM loaders are sensitive to path.
             // We'll verify if 'sherpa-ncnn.js' or others are needed alongside.
            copyFileSync(join(ncnnDir, file), join(distDir, file));
            console.log(`Copied ${file} to dist/`);
        }
    }
} else {
    console.warn('Could not find sherpa-ncnn module to copy assets from.');
}

console.log('Build complete.');
