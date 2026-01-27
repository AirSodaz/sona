#!/usr/bin/env node
/**
 * Sona - Sherpa-onnx/ncnn Speech Recognition Sidecar
 * 
 * Supports two operational modes:
 * - Mode A (Stream): Reads PCM audio from stdin, outputs JSON lines
 * - Mode B (Batch): Processes audio file, outputs full segment array
 * 
 * Supports two engines:
 * - sherpa-onnx-node: For CPU inference (ONNX models)
 * - sherpa-ncnn: For GPU inference (NCNN models with Vulkan/CoreML support)
 */

import { spawn, execSync } from 'child_process';
import os from 'os';
import { createReadStream, existsSync, statSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { Readable, Transform } from 'stream';
import { randomUUID } from 'crypto';


// For ES modules
const __scriptFile = fileURLToPath(import.meta.url);
const __scriptDir = dirname(__scriptFile);
const Seven = require('node-7z');

// Parse command line arguments
function parseArgs() {
    const args = process.argv.slice(2);
    const options = {
        mode: 'stream',
        file: null,
        modelPath: null,
        sampleRate: 16000,
        enableITN: true,
        provider: null, // auto-detect if null
        allowMock: false,
        allowMock: false,
        numThreads: null,
        targetDir: null,
        targetDir: null,
        itnModel: null,
        punctuationModel: null,
    };

    for (let i = 0; i < args.length; i++) {
        switch (args[i]) {
            case '--mode':
                options.mode = args[++i];
                break;
            case '--file':
                options.file = args[++i];
                break;
            case '--model-path':
                options.modelPath = args[++i];
                break;
            case '--sample-rate':
                options.sampleRate = parseInt(args[++i], 10);
                break;
            case '--enable-itn':
                options.enableITN = args[++i] === 'true';
                break;
            case '--provider':
                options.provider = args[++i];
                break;
            case '--allow-mock':
                options.allowMock = args[++i] === 'true';
                break;
            case '--num-threads':
                options.numThreads = parseInt(args[++i], 10);
                break;
            case '--target-dir':
                options.targetDir = args[++i];
                break;
            case '--itn-model':
                options.itnModel = args[++i];
                break;
            case '--punctuation-model':
                options.punctuationModel = args[++i];
                break;
        }
    }

    return options;
}

// Get ffmpeg path
async function getFFmpegPath() {
    const localOne = join(__scriptDir, 'ffmpeg.exe');
    if (existsSync(localOne)) return localOne;
    const localTwo = join(__scriptDir, 'ffmpeg');
    if (existsSync(localTwo)) return localTwo;

    const cwdOne = join(process.cwd(), 'ffmpeg.exe');
    if (existsSync(cwdOne)) return cwdOne;

    try {
        const ffmpegStatic = await import('ffmpeg-static');
        return ffmpegStatic.default;
    } catch {
        return 'ffmpeg';
    }
}

// Convert audio file to 16kHz mono WAV using ffmpeg
async function convertToWav(inputPath, ffmpegPath) {
    return new Promise((resolve, reject) => {
        const chunks = [];
        const ffmpeg = spawn(ffmpegPath, [
            '-i', inputPath,
            '-f', 's16le', '-acodec', 'pcm_s16le', '-ar', '16000', '-ac', '1', '-'
        ]);

        ffmpeg.stdout.on('data', (chunk) => chunks.push(chunk));
        ffmpeg.stderr.on('data', () => { });
        ffmpeg.on('close', (code) => {
            if (code === 0) resolve(Buffer.concat(chunks));
            else reject(new Error(`FFmpeg exited with code ${code}`));
        });
        ffmpeg.on('error', reject);
    });
}

// Create NCNN Recognizer (GPU/Vulkan/CoreML)
async function createNcnnRecognizer(modelConfig, enableITN, numThreads) {
    console.error(`Initializing NcnnRecognizer (Threads: ${numThreads})...`);
    try {
        const sherpaNcnn = await import('sherpa-ncnn');

        // Configuration for sherpa-ncnn
        const config = {
            featConfig: {
                samplingRate: 16000,
                featureDim: 80,
            },
            modelConfig: {
                ...modelConfig,
                useVulkanCompute: 1, // Enable GPU
                numThreads: numThreads,
            },
            decoderConfig: {
                decodingMethod: 'greedy_search',
                numActivePaths: 4,
            },
            enableEndpoint: 1,
            rule1MinTrailingSilence: 2.4,
            rule2MinTrailingSilence: 2.4,
            rule3MinUtternceLength: 300, // Large number to mimic sherpa-onnx defaults? Or 0?
        };

        const recognizer = sherpaNcnn.createRecognizer(config);

        // Wrap to match interface
        return {
            recognizer: {
                createStream: () => recognizer.createStream(),
                isReady: (stream) => recognizer.isReady(stream),
                decode: (stream) => recognizer.decode(stream),
                isEndpoint: (stream) => recognizer.isEndpoint(stream),
                reset: (stream) => recognizer.reset(stream),
                getResult: (stream) => {
                    const text = recognizer.getResult(stream);
                    return { text: text }; // Wrap string in object
                }
            },
            type: 'online-ncnn',
            supportsInternalITN: false
        };
    } catch (e) {
        throw new Error(`Failed to initialize sherpa-ncnn: ${e.message}`);
    }
}

// Create ONNX Recognizer (CPU)
async function createOnnxRecognizer(modelConfig, enableITN, numThreads, itnModel) {
    console.error(`Initializing OnnxRecognizer (CPU, Threads: ${numThreads})...`);
    try {
        const sherpaModule = await import('sherpa-onnx-node');
        const sherpa = sherpaModule.default || sherpaModule;

        // Offline models (SenseVoice, Whisper)
        if (modelConfig.senseVoice || modelConfig.whisper) {
            const config = {
                featConfig: { sampleRate: 16000, featureDim: 80 },
                modelConfig: modelConfig,
                decodingMethod: 'greedy_search',
                maxActivePaths: 4,
            };

            if (itnModel) {
                const paths = itnModel.split(',');
                const validPaths = paths.filter(p => existsSync(p.trim()));
                if (validPaths.length > 0) {
                    console.error(`[Sidecar] Using ITN models (Offline): ${validPaths.join(',')}`);
                    config.ruleFsts = validPaths.join(',');
                }
            }

            return {
                recognizer: new sherpa.OfflineRecognizer(config),
                type: 'offline',
                supportsInternalITN: true
            };
        }

        // Online models
        const config = {
            featConfig: { sampleRate: 16000, featureDim: 80 },
            modelConfig: modelConfig,
            decodingMethod: 'greedy_search',
            maxActivePaths: 4,
            enableEndpoint: true,
            endpointConfig: {
                rule1: { minTrailingSilence: 2.4, minUtteranceLength: 0 },
                rule2: { minTrailingSilence: 2.4, minUtteranceLength: 0 },
                rule3: { minTrailingSilence: 0, minUtteranceLength: 300 },
            },
        };

        if (itnModel) {
            const paths = itnModel.split(',');
            const validPaths = paths.filter(p => existsSync(p.trim()));
            if (validPaths.length > 0) {
                console.error(`[Sidecar] Using ITN models: ${validPaths.join(',')}`);
                config.ruleFsts = validPaths.join(',');
            }
        }

        const supportsInternalITN = !!config.ruleFsts || !!(modelConfig.senseVoice && modelConfig.senseVoice.useInverseTextNormalization);

        return {
            recognizer: new sherpa.OnlineRecognizer(config),
            type: 'online',
            supportsInternalITN: false
        };
    } catch (e) {
        throw new Error(`Failed to initialize sherpa-onnx: ${e.message}`);
    }
}

// Find model configuration and detect type
function findModelConfig(modelPath, enableITN, numThreads) {
    if (!existsSync(modelPath)) return null;

    try {
        const files = readdirSync(modelPath);
        const tokens = files.find(f => f === 'tokens.txt');

        if (!tokens) return null; // All models need tokens.txt

        // 1. Check for NCNN files (.bin + .param)
        // Expected: encoder_jit_trace-pnnx.ncnn.bin/param, decoder..., joiner...
        const hasNcnn = files.some(f => f.endsWith('.ncnn.bin'));

        if (hasNcnn) {
            // NCNN Logic
            const findNcnnFile = (pattern) => {
                const f = files.find(file => file.includes(pattern) && (file.endsWith('.bin') || file.endsWith('.param')));
                return f ? join(modelPath, f) : null;
            };

            const encoderBin = findNcnnFile('encoder_jit_trace-pnnx.ncnn.bin');
            const encoderParam = findNcnnFile('encoder_jit_trace-pnnx.ncnn.param');
            const decoderBin = findNcnnFile('decoder_jit_trace-pnnx.ncnn.bin');
            const decoderParam = findNcnnFile('decoder_jit_trace-pnnx.ncnn.param');
            const joinerBin = findNcnnFile('joiner_jit_trace-pnnx.ncnn.bin');
            const joinerParam = findNcnnFile('joiner_jit_trace-pnnx.ncnn.param');

            if (encoderBin && encoderParam && decoderBin && decoderParam && joinerBin && joinerParam) {
                return {
                    type: 'ncnn',
                    config: {
                        encoderBin, encoderParam,
                        decoderBin, decoderParam,
                        joinerBin, joinerParam,
                        tokens: join(modelPath, tokens)
                    }
                };
            }
        }

        // 2. Check for ONNX files
        const findBestMatch = (prefix) => {
            const candidates = files.filter(f => f.includes(prefix) && f.endsWith('.onnx'));
            if (candidates.length === 0) return null;
            const int8 = candidates.find(f => f.includes('int8'));
            if (int8) return join(modelPath, int8);
            return join(modelPath, candidates[0]);
        };

        const encoder = findBestMatch('encoder');
        const decoder = findBestMatch('decoder');
        const joiner = findBestMatch('joiner');

        if (encoder && decoder) {
            const baseConfig = {
                tokens: join(modelPath, tokens),
                numThreads: numThreads,
                provider: 'cpu', // ONNX is CPU only now per requirement
                debug: false,
            };

            if (joiner) {
                return {
                    type: 'onnx',
                    config: { ...baseConfig, transducer: { encoder, decoder, joiner } }
                };
            } else {
                return {
                    type: 'onnx',
                    config: { ...baseConfig, paraformer: { encoder, decoder } }
                };
            }
        }

        // SenseVoice / Whisper (Offline ONNX)
        const model = findBestMatch('model');
        if (model && !encoder) {
            return {
                type: 'onnx',
                config: {
                    tokens: join(modelPath, tokens),
                    numThreads: numThreads,
                    provider: 'cpu',
                    debug: false,
                    senseVoice: {
                        model: model,
                        language: '',
                        useInverseTextNormalization: enableITN ? 1 : 0,
                    }
                }
            };
        }

    } catch (error) {
        console.error(`Error scanning model directory: ${error.message}`);
    }

    return null;
}

// Create Punctuation Model
async function createPunctuation(modelPath) {
    if (!modelPath || !existsSync(modelPath)) return null;
    console.error(`[Sidecar] Initializing Punctuation Model: ${modelPath}`);
    try {
        const sherpaModule = await import('sherpa-onnx-node');
        const sherpa = sherpaModule.default || sherpaModule;

        const files = readdirSync(modelPath);
        const modelFile = files.find(f => f.endsWith('.onnx'));

        if (!modelFile) {
            console.error('[Sidecar] No .onnx file found in punctuation model directory');
            return null;
        }

        const config = {
            model: {
                ctTransformer: join(modelPath, modelFile),
                numThreads: 1,
                debug: false,
                provider: 'cpu'
            }
        };

        return new sherpa.OfflinePunctuation(config);
    } catch (e) {
        console.error(`[Sidecar] Failed to initialize punctuation: ${e.message}`);
        return null;
    }
}

// Text post-processing
function postProcessText(text) {
    if (!text) return "";
    if (/[a-zA-Z]/.test(text) && text === text.toUpperCase()) {
        const lower = text.toLowerCase();
        return lower.charAt(0).toUpperCase() + lower.slice(1);
    }
    return text;
}



// Process Stream
async function processStream(recognizer, sampleRate, punctuation) {
    const stream = recognizer.createStream();
    let totalSamples = 0;
    let segmentStartTime = 0;
    let currentSegmentId = randomUUID();

    process.stdin.on('data', (chunk) => {
        const samples = new Float32Array(chunk.length / 2);
        for (let i = 0; i < samples.length; i++) {
            const int16 = chunk.readInt16LE(i * 2);
            samples[i] = int16 / 32768.0;
        }

        if (recognizer.isNcnn) {
            stream.acceptWaveform(sampleRate, samples);
        } else {
            stream.acceptWaveform({ samples: samples, sampleRate: sampleRate });
        }

        totalSamples += samples.length;

        while (recognizer.isReady(stream)) {
            recognizer.decode(stream);
        }

        const result = recognizer.getResult(stream);
        const currentTime = totalSamples / sampleRate;

        if (result.text.trim()) {
            let text = postProcessText(result.text.trim());
            const output = {
                id: currentSegmentId,
                text: text,
                start: segmentStartTime,
                end: currentTime,
                isFinal: false,
            };
            console.log(JSON.stringify(output));
        }

        if (recognizer.isEndpoint(stream)) {
            const finalResult = recognizer.getResult(stream);
            if (finalResult.text.trim()) {
                let text = postProcessText(finalResult.text.trim());
                if (punctuation) text = punctuation.addPunct(text);
                const output = {
                    id: currentSegmentId,
                    text: text,
                    start: segmentStartTime,
                    end: currentTime,
                    isFinal: true,
                };
                console.log(JSON.stringify(output));
            }
            recognizer.reset(stream);
            segmentStartTime = currentTime;
            currentSegmentId = randomUUID();
        }
    });

    process.stdin.on('end', () => {
        const finalResult = recognizer.getResult(stream);
        const currentTime = totalSamples / sampleRate;
        if (finalResult.text.trim()) {
            let text = postProcessText(finalResult.text.trim());
            if (punctuation) text = punctuation.addPunct(text);
            console.log(JSON.stringify({
                id: currentSegmentId,
                text: text,
                start: segmentStartTime,
                end: currentTime,
                isFinal: true,
            }));
        }
        console.log(JSON.stringify({ done: true }));
        process.exit(0);
    });
}

// Process Batch
async function processBatch(recognizer, filePath, ffmpegPath, sampleRate, punctuation) {
    if (!existsSync(filePath)) throw new Error(`File not found: ${filePath}`);

    try {
        console.error('Converting audio file...');
        const pcmData = await convertToWav(filePath, ffmpegPath);
        const samples = new Float32Array(pcmData.length / 2);
        for (let i = 0; i < samples.length; i++) {
            const int16 = pcmData.readInt16LE(i * 2);
            samples[i] = int16 / 32768.0;
        }

        console.error(`Processing ${samples.length} samples...`);
        const stream = recognizer.createStream();
        const segments = [];
        let segmentStartTime = 0;
        const chunkSize = sampleRate * 0.5;

        for (let i = 0; i < samples.length; i += chunkSize) {
            const chunk = samples.slice(i, Math.min(i + chunkSize, samples.length));

            // UNIFIED call needed here
            if (recognizer.isNcnn) {
                stream.acceptWaveform(sampleRate, chunk);
            } else {
                stream.acceptWaveform({ samples: chunk, sampleRate: sampleRate });
            }

            while (recognizer.isReady(stream)) recognizer.decode(stream);

            if (recognizer.isEndpoint(stream)) {
                const result = recognizer.getResult(stream);
                const currentTime = i / sampleRate;
                if (result.text.trim()) {
                    let text = postProcessText(result.text.trim());
                    if (punctuation) text = punctuation.addPunct(text);
                    segments.push({
                        id: randomUUID(),
                        text: text,
                        start: segmentStartTime,
                        end: currentTime,
                        isFinal: true,
                    });
                }
                recognizer.reset(stream);
                segmentStartTime = currentTime;
            }

            if (i % (sampleRate * 5) === 0) {
                const progress = Math.round((i / samples.length) * 100);
                console.error(`Progress: ${progress}%`);
            }
        }

        const finalResult = recognizer.getResult(stream);
        const totalDuration = samples.length / sampleRate;
        if (finalResult.text.trim()) {
            let text = postProcessText(finalResult.text.trim());
            if (punctuation) text = punctuation.addPunct(text);
            segments.push({
                id: randomUUID(),
                text: text,
                start: segmentStartTime,
                end: totalDuration,
                isFinal: true,
            });
        }
        console.log(JSON.stringify(segments, null, 2));

    } catch (error) {
        throw error;
    }
}

async function processExtraction(archivePath, targetDir) {
    const sevenZipPath = join(__scriptDir, process.platform === 'win32' ? '7za.exe' : '7za');

    if (!existsSync(sevenZipPath)) {
        throw new Error(`7zip binary not found at ${sevenZipPath}`);
    }

    if (!existsSync(archivePath)) {
        throw new Error(`Archive not found: ${archivePath}`);
    }

    // Ensure target dir exists
    if (!existsSync(targetDir)) {
        // We can create it or let 7zip do it, but careful with paths
        // mkdirSync(targetDir, { recursive: true });
    }

    const isTarBz2 = archivePath.toLowerCase().endsWith('.tar.bz2');

    return new Promise((resolve, reject) => {
        const stream = Seven.extractFull(archivePath, targetDir, {
            $bin: sevenZipPath,
            $progress: true,
            // recursive: true // extractFull is recursive
        });

        stream.on('progress', (progress) => {
            let percentage = progress.percent;
            // If it's a tar.bz2, this is just step 1 (bz2 -> tar), so scale 0-50%
            if (isTarBz2) {
                percentage = Math.round(percentage / 2);
            }
            console.log(JSON.stringify({
                percentage: percentage,
                status: progress.file || 'Extracting...',
                type: 'progress'
            }));
        });

        stream.on('end', async () => {
            // Check if we extracted a .tar file (common with .tar.bz2)
            const files = readdirSync(targetDir);
            const tarFile = files.find(f => f.endsWith('.tar'));

            if (tarFile) {
                const tarPath = join(targetDir, tarFile);
                console.error(`[Sidecar] Found .tar file after extraction: ${tarFile}. Extracting again...`);

                // Extract the tar
                try {
                    await new Promise((resolveTar, rejectTar) => {
                        const streamTar = Seven.extractFull(tarPath, targetDir, {
                            $bin: sevenZipPath,
                            $progress: true
                        });

                        streamTar.on('progress', (p) => {
                            // Step 2: tar -> files, scale 50-100%
                            const percentage = 50 + Math.round(p.percent / 2);
                            console.log(JSON.stringify({
                                percentage: percentage,
                                status: `Extracting tar: ${p.file || ''}`,
                                type: 'progress'
                            }));
                        });

                        streamTar.on('end', () => resolveTar());
                        streamTar.on('error', (err) => rejectTar(err));
                    });

                    // Delete the intermediate .tar
                    try {
                        const fs = require('fs');
                        fs.unlinkSync(tarPath);
                    } catch (e) { }

                    console.log(JSON.stringify({ done: true }));
                    resolve();
                } catch (e) {
                    reject(e);
                }
            } else {
                console.log(JSON.stringify({ done: true }));
                resolve();
            }
        });

        stream.on('error', (err) => {
            reject(err);
        });
    });
}




async function processBatchOffline(recognizer, filePath, ffmpegPath, sampleRate, punctuation) {
    if (!existsSync(filePath)) throw new Error(`File not found: ${filePath}`);

    try {
        console.error('Converting audio file (Offline)...');
        const pcmData = await convertToWav(filePath, ffmpegPath);
        const samples = new Float32Array(pcmData.length / 2);
        for (let i = 0; i < samples.length; i++) {
            samples[i] = pcmData.readInt16LE(i * 2) / 32768.0;
        }

        console.error(`Processing ${samples.length} samples with OfflineRecognizer...`);
        const stream = recognizer.createStream();
        stream.acceptWaveform({ samples: samples, sampleRate: sampleRate });

        recognizer.decode(stream);
        const result = recognizer.getResult(stream);
        const segments = [];

        if (result.text && result.text.trim()) {
            let text = postProcessText(result.text.trim());
            if (punctuation) text = punctuation.addPunct(text);
            segments.push({
                id: randomUUID(),
                text: text,
                start: 0,
                end: samples.length / sampleRate,
                isFinal: true,
                tokens: result.tokens || [],
                timestamps: result.timestamps || []
            });
        }
        console.log(JSON.stringify(segments, null, 2));
    } catch (error) {
        throw error;
    }
}

// Get number of physical cores
function getPhysicalCores() {
    const platform = process.platform;
    try {
        if (platform === 'linux') {
            const output = execSync("lscpu -p | grep -E -v '^#' | sort -u -t, -k 2,4 | wc -l").toString().trim();
            const cores = parseInt(output, 10);
            if (!isNaN(cores) && cores > 0) return cores;
        } else if (platform === 'darwin') {
            const output = execSync("sysctl -n hw.physicalcpu").toString().trim();
            const cores = parseInt(output, 10);
            if (!isNaN(cores) && cores > 0) return cores;
        } else if (platform === 'win32') {
            const output = execSync("wmic cpu get NumberOfCores").toString().trim();
            const lines = output.split('\n');
            let total = 0;
            for (const line of lines) {
                const val = parseInt(line.trim(), 10);
                if (!isNaN(val)) total += val;
            }
            if (total > 0) return total;
        }
    } catch (e) {
    }

    return Math.ceil(os.cpus().length / 2);
}

// Calculate optimal thread count based on physical cores
function calculateNumThreads(providedValue) {
    if (providedValue) return providedValue;

    const physicalCores = getPhysicalCores();
    console.error(`[Sidecar] Physical cores detected: ${physicalCores}`);

    let numThreads;
    if (physicalCores <= 4) {
        numThreads = physicalCores - 1;
    } else {
        numThreads = physicalCores - 2;
    }

    return Math.max(1, numThreads);
}

// Ensure Library Path is set (DYLD_LIBRARY_PATH on macOS, LD_LIBRARY_PATH on Linux)
async function ensureLibraryPath() {
    if (process.platform !== 'darwin' && process.platform !== 'linux') return false;
    if (process.env.SONA_LIB_FIXED) return false;

    const envVar = process.platform === 'darwin' ? 'DYLD_LIBRARY_PATH' : 'LD_LIBRARY_PATH';
    const libName = process.platform === 'darwin' ? 'libsherpa-onnx-c-api.dylib' : 'libsherpa-onnx-c-api.so';
    const arch = process.arch;
    const packageName = `sherpa-onnx-${process.platform}-${arch}`;

    let targetPath = null;

    // 1. Check for flattened libraries in script directory (bundled/production)
    const flattenedLib = join(__scriptDir, libName);
    if (existsSync(flattenedLib)) {
        targetPath = __scriptDir;
    }

    // 2. Look for node_modules in likely locations (dev/source)
    if (!targetPath) {
        const candidates = [
            join(__scriptDir, '../../node_modules'),
            join(__scriptDir, 'node_modules'),
            join(__scriptDir, '../node_modules'),
        ];

        for (const base of candidates) {
            const p = join(base, packageName);
            if (existsSync(p)) {
                targetPath = p;
                break;
            }
        }
    }

    if (!targetPath) {
        return false;
    }

    const currentPath = process.env[envVar] || '';
    if (currentPath.includes(targetPath)) {
        return false; // Already set
    }

    const newPath = currentPath
        ? `${targetPath}:${currentPath}`
        : targetPath;

    console.error(`[Sidecar] Setting ${envVar} to ${targetPath} and respawning...`);

    const newEnv = {
        ...process.env,
        [envVar]: newPath,
        SONA_LIB_FIXED: '1'
    };

    const child = spawn(process.execPath, process.argv.slice(1), {
        env: newEnv,
        stdio: 'inherit'
    });

    child.on('close', (code) => {
        process.exit(code);
    });

    // Forward signals
    ['SIGINT', 'SIGTERM', 'SIGHUP'].forEach(signal => {
        process.on(signal, () => {
            if (!child.killed) child.kill(signal);
        });
    });

    return true;
}


// Refactored main
async function main() {
    if (await ensureLibraryPath()) return;
    const options = parseArgs();

    if (options.mode === 'extract') {
        if (!options.file || !options.targetDir) {
            console.error(JSON.stringify({ error: 'File and Target Directory required for extraction' }));
            process.exit(1);
        }
        try {
            await processExtraction(options.file, options.targetDir);
            process.exit(0);
        } catch (e) {
            console.error(JSON.stringify({ error: e.message || String(e) }));
            process.exit(1);
        }
    }


    if (!options.modelPath) {
        console.error(JSON.stringify({ error: 'Model path is required.' }));
        process.exit(1);
    }

    const ffmpegPath = await getFFmpegPath();
    const numThreads = calculateNumThreads(options.numThreads);

    // Mock Mode
    const useMock = (options.allowMock && !existsSync(options.modelPath)) || process.env.SONA_MOCK === '1';
    if (useMock) {
        console.error('Running in mock mode');
        if (options.mode === 'batch' && options.file) {
            console.log(JSON.stringify([
                { id: randomUUID(), start: 0, end: 1, text: 'Mock transcript.', isFinal: true }
            ], null, 2));
        } else {
            console.log(JSON.stringify({
                id: randomUUID(), text: 'Mock streaming', start: 0, end: 1, isFinal: true
            }));
        }
        process.exit(0);
    }

    try {
        const found = findModelConfig(options.modelPath, options.enableITN, numThreads);
        if (!found) throw new Error('Could not find valid model configuration (NCNN or ONNX)');

        let recognizerObj;

        if (found.type === 'ncnn') {
            recognizerObj = await createNcnnRecognizer(found.config, options.enableITN, numThreads);
            // Flag to helper know it is NCNN for stream API diffs
            recognizerObj.recognizer.isNcnn = true;
        } else {
            recognizerObj = await createOnnxRecognizer(found.config, options.enableITN, numThreads, options.itnModel);
        }

        const { recognizer, type, supportsInternalITN } = recognizerObj;

        // Unified Stream Processor Wrapper
        // Avoid spreading (...recognizer) on class instances as it strips prototype methods
        const unifiedRecognizer = recognizer;
        // Ensure isNcnn flag is present (already set for NCNN path, undefined for ONNX)
        unifiedRecognizer.isNcnn = !!recognizer.isNcnn;

        const punctuation = await createPunctuation(options.punctuationModel);

        if (options.mode === 'batch') {
            if (!options.file) throw new Error('File path required for batch mode');
            if (type === 'offline') {
                await processBatchOffline(unifiedRecognizer, options.file, ffmpegPath, options.sampleRate, punctuation);
            } else {
                await processBatch(unifiedRecognizer, options.file, ffmpegPath, options.sampleRate, punctuation);
            }
        } else {
            if (type === 'offline') {
                console.error(JSON.stringify({ error: 'Streaming mode not supported for offline model.' }));
                process.exit(1);
            }

            await processStream(unifiedRecognizer, options.sampleRate, punctuation);
        }

    } catch (error) {
        console.error(JSON.stringify({ error: error.message || String(error) }));
        process.exit(1);
    }
}

main();
