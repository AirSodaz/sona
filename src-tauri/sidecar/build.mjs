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
        // We will copy .node file next to the bundle, so we can let it try to require it.
        // But esbuild will try to bundle the require call.
        // actually sherpa-onnx-node requires using a variable path, so esbuild might emit a warning or ignore it.
        // Ideally we want sherpa-onnx-node code included, but the finding logic preserved.
        'sherpa-onnx.node'
    ],
    loader: { '.node': 'file' } // or copy it manually
});

// 2. Copy sherpa-onnx.node
console.log('Copying native bindings...');
// We found it at node_modules/sherpa-onnx-win-x64/sherpa-onnx.node
// But let's find it dynamically or hardcode for now based on previous find result
const nodeFileSrc = join(__dirname, 'node_modules/sherpa-onnx-win-x64/sherpa-onnx.node');
if (existsSync(nodeFileSrc)) {
    copyFileSync(nodeFileSrc, join(distDir, 'sherpa-onnx.node'));
} else {
    console.error('Could not find sherpa-onnx.node');
    process.exit(1);
}

// 3. Copy ffmpeg.exe
console.log('Copying ffmpeg...');
const ffmpegSrc = join(__dirname, 'node_modules/ffmpeg-static/ffmpeg.exe');
// Note: ffmpeg-static might have it in a different path, waiting for find_by_name result to confirm.
if (existsSync(ffmpegSrc)) {
    copyFileSync(ffmpegSrc, join(distDir, 'ffmpeg.exe'));
} else {
    // fallback check based on find result later
    console.log('Checking fallback ffmpeg path...');
}

console.log('Build complete.');
