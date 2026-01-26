#!/usr/bin/env node
/**
 * Sona - Sherpa-onnx Speech Recognition Sidecar
 * 
 * Supports two operational modes:
 * - Mode A (Stream): Reads PCM audio from stdin, outputs JSON lines
 * - Mode B (Batch): Processes audio file, outputs full segment array
 * 
 * Usage:
 *   node sherpa-recognizer.js --mode stream --model-path /path/to/model
 *   node sherpa-recognizer.js --mode batch --file /path/to/audio.mp3 --model-path /path/to/model
 */

import { spawn } from 'child_process';
import { createReadStream, existsSync, statSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { Readable, Transform } from 'stream';
import { randomUUID } from 'crypto';
import Nzh from 'nzh';

// For ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Parse command line arguments
function parseArgs() {
    const args = process.argv.slice(2);
    const options = {
        mode: 'stream',
        file: null,
        modelPath: null,
        sampleRate: 16000,
        enableITN: true,
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
        }
    }

    return options;
}

// Get ffmpeg path (from ffmpeg-static or system)
async function getFFmpegPath() {
    try {
        const ffmpegStatic = await import('ffmpeg-static');
        return ffmpegStatic.default;
    } catch {
        // Fall back to system ffmpeg
        return 'ffmpeg';
    }
}

// Convert audio file to 16kHz mono WAV using ffmpeg
async function convertToWav(inputPath, ffmpegPath) {
    return new Promise((resolve, reject) => {
        const chunks = [];

        const ffmpeg = spawn(ffmpegPath, [
            '-i', inputPath,
            '-f', 's16le',        // 16-bit signed little-endian PCM
            '-acodec', 'pcm_s16le',
            '-ar', '16000',       // 16kHz sample rate
            '-ac', '1',           // Mono
            '-'                   // Output to stdout
        ]);

        ffmpeg.stdout.on('data', (chunk) => {
            chunks.push(chunk);
        });

        ffmpeg.stderr.on('data', (data) => {
            // FFmpeg outputs info to stderr, ignore it
        });

        ffmpeg.on('close', (code) => {
            if (code === 0) {
                resolve(Buffer.concat(chunks));
            } else {
                reject(new Error(`FFmpeg exited with code ${code}`));
            }
        });

        ffmpeg.on('error', reject);
    });
}

// Create sherpa-onnx recognizer (Online or Offline)
async function createRecognizer(modelPath, enableITN) {
    try {
        const sherpaModule = await import('sherpa-onnx-node');
        const sherpa = sherpaModule.default || sherpaModule;

        const modelConfig = findModelConfig(modelPath, enableITN);
        if (!modelConfig) {
            throw new Error(`Could not find valid model configuration in: ${modelPath}`);
        }

        // Check if it's an offline config (SenseVoice, Whisper, etc.)
        if (modelConfig.senseVoice || modelConfig.whisper) {
            console.error('Initializing OfflineRecognizer...');
            const config = {
                featConfig: {
                    sampleRate: 16000,
                    featureDim: 80,
                },
                modelConfig: modelConfig,
                // Offline recognizer options
                decodingMethod: 'greedy_search',
                maxActivePaths: 4,
            };

            return {
                recognizer: new sherpa.OfflineRecognizer(config),
                type: 'offline',
                supportsInternalITN: true // SenseVoice/Whisper usually support internal ITN if configured
            };
        }

        // Default to Online
        console.error('Initializing OnlineRecognizer...');
        const config = {
            featConfig: {
                sampleRate: 16000,
                featureDim: 80,
            },
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

        // Check if ITN is enabled but handled internally (e.g. by SenseVoice)
        // or if we should use our JS fallback.
        const supportsInternalITN = !!(modelConfig.senseVoice && modelConfig.senseVoice.useInverseTextNormalization);

        if (enableITN && !supportsInternalITN) {
            console.error('ITN Enabled (using JS post-processing).');
        } else if (enableITN && supportsInternalITN) {
            console.error('ITN Enabled (using internal model capability).');
        }

        return {
            recognizer: new sherpa.OnlineRecognizer(config),
            type: 'online',
            supportsInternalITN: false // Online models here don't have internal ITN configured yet
        };

    } catch (error) {
        console.error(JSON.stringify({ error: `Failed to create recognizer: ${error.message}` }));
        process.exit(1);
    }
}

// Find model configuration based on model directory contents
function findModelConfig(modelPath, enableITN) {
    if (!existsSync(modelPath)) {
        return null;
    }

    try {
        const files = readdirSync(modelPath);

        // Helper to find file by prefix
        const findBestMatch = (prefix) => {
            const candidates = files.filter(f => f.startsWith(prefix) && f.endsWith('.onnx'));
            if (candidates.length === 0) return null;
            const int8 = candidates.find(f => f.includes('int8'));
            if (int8) return join(modelPath, int8);
            return join(modelPath, candidates[0]);
        };

        // 1. Check for Online Models (Transducer, Paraformer-Streaming)
        // They typically have encoder / decoder
        const encoder = findBestMatch('encoder');
        const decoder = findBestMatch('decoder');
        const joiner = findBestMatch('joiner');
        const tokens = files.find(f => f === 'tokens.txt');



        if (tokens && encoder && decoder) {
            const baseConfig = {
                tokens: join(modelPath, tokens),
                numThreads: 2,
                provider: 'cpu',
                debug: false,
            };

            if (joiner) {
                // Transducer (Online)
                return {
                    ...baseConfig,
                    transducer: { encoder, decoder, joiner }
                };
            } else {
                // Paraformer (Online check - Paraformer can be offline too but usually has same structure? 
                // Actually Paraformer-Online and Offline have different structures usually, 
                // but let's assume if it has encoder/decoder it's Online-compatible or we treat as Online for now)
                // Note: Offline Paraformer usually has 'model.onnx' in older versions or same structure.
                // But specifically for detecting SenseVoice vs others.

                // Let's stick to existing logic for Online Paraformer
                return {
                    ...baseConfig,
                    paraformer: { encoder, decoder }
                };
            }
        }

        // 2. Check for Offline Models (SenseVoice, Whisper, etc.)

        // SenseVoice: model.onnx + tokens.txt
        const model = findBestMatch('model');
        if (tokens && model && !encoder) {
            // SenseVoice detection
            return {
                tokens: join(modelPath, tokens),
                numThreads: 2,
                provider: 'cpu',
                debug: false,
                senseVoice: {
                    model: model,
                    language: '', // auto-detect
                    useInverseTextNormalization: enableITN ? 1 : 0,
                }
            };
        }

    } catch (error) {
        console.error(`Error scanning model directory: ${error.message}`);
    }

    return null;
}




// Helper to fix ALL CAPS text from some models
function postProcessText(text) {
    if (!text) return "";

    // If text contains letters and is all uppercase
    // We assume mixed content (e.g. "HELLO WORLD") is what we want to fix
    // But we shouldn't touch Chinese or mixed Chinese/English if the English part is fine.
    // However, if the English part is ALL CAPS, we should fix it.

    // Simple heuristic: if the string equals its uppercase version (and has letters), it is ALL CAPS.
    if (/[a-zA-Z]/.test(text) && text === text.toUpperCase()) {
        const lower = text.toLowerCase();
        // Capitalize first letter
        return lower.charAt(0).toUpperCase() + lower.slice(1);
    }
    return text;
}

// Apply Inverse Text Normalization (Chinese numbers to digits)
const nzhcn = Nzh.cn;
function applyITN(text) {
    if (!text) return "";

    // Match Chinese number patterns and convert them
    // Pattern matches sequences of Chinese number characters
    const chineseNumPattern = /[零一二三四五六七八九十百千万亿点两]+/g;

    return text.replace(chineseNumPattern, (match) => {
        try {
            // Try to decode the Chinese number
            const decoded = nzhcn.decodeS(match);
            // If successful and different from input, use the decoded version
            if (decoded && decoded !== match) {
                return decoded;
            }
        } catch (e) {
            // If decoding fails, keep original
        }
        return match;
    });
}

// Process audio stream (Mode A)
async function processStream(recognizer, sampleRate, enableITN) {
    const stream = recognizer.createStream();
    let totalSamples = 0;
    let segmentStartTime = 0;
    let currentSegmentId = randomUUID();

    // Read 16-bit PCM from stdin
    process.stdin.on('data', (chunk) => {
        // Convert Buffer to Float32Array
        const samples = new Float32Array(chunk.length / 2);
        for (let i = 0; i < samples.length; i++) {
            const int16 = chunk.readInt16LE(i * 2);
            samples[i] = int16 / 32768.0;
        }

        stream.acceptWaveform({ samples: samples, sampleRate: sampleRate });
        totalSamples += samples.length;

        while (recognizer.isReady(stream)) {
            recognizer.decode(stream);
        }

        const result = recognizer.getResult(stream);
        const currentTime = totalSamples / sampleRate;

        if (result.text.trim()) {
            let text = postProcessText(result.text.trim());
            if (enableITN) text = applyITN(text);
            const output = {
                id: currentSegmentId,
                text: text,
                start: segmentStartTime,
                end: currentTime,
                isFinal: false,
            };


            console.log(JSON.stringify(output));
        }

        // Check for endpoint (sentence end)
        if (recognizer.isEndpoint(stream)) {
            const finalResult = recognizer.getResult(stream);

            if (finalResult.text.trim()) {
                let text = postProcessText(finalResult.text.trim());
                if (enableITN) text = applyITN(text);
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
        // Process any remaining audio
        const finalResult = recognizer.getResult(stream);
        const currentTime = totalSamples / sampleRate;

        if (finalResult.text.trim()) {
            let text = postProcessText(finalResult.text.trim());
            if (enableITN) text = applyITN(text);
            const output = {
                id: currentSegmentId,
                text: text,
                start: segmentStartTime,
                end: currentTime,
                isFinal: true,
            };


            console.log(JSON.stringify(output));
        }

        console.log(JSON.stringify({ done: true }));
        process.exit(0);
    });
}

// Process audio file (Mode B)
async function processBatch(recognizer, filePath, ffmpegPath, sampleRate, enableITN) {
    if (!existsSync(filePath)) {
        console.error(JSON.stringify({ error: `File not found: ${filePath}` }));
        process.exit(1);
    }

    try {
        // Convert to PCM
        console.error('Converting audio file...');
        const pcmData = await convertToWav(filePath, ffmpegPath);

        // Convert to Float32Array
        const samples = new Float32Array(pcmData.length / 2);
        for (let i = 0; i < samples.length; i++) {
            const int16 = pcmData.readInt16LE(i * 2);
            samples[i] = int16 / 32768.0;
        }

        console.error(`Processing ${samples.length} samples...`);

        // Process with recognizer
        const stream = recognizer.createStream();
        const segments = [];
        let segmentStartTime = 0;

        // Process in chunks
        const chunkSize = sampleRate * 0.5; // 500ms chunks

        for (let i = 0; i < samples.length; i += chunkSize) {
            const chunk = samples.slice(i, Math.min(i + chunkSize, samples.length));
            stream.acceptWaveform({ samples: chunk, sampleRate: sampleRate });

            while (recognizer.isReady(stream)) {
                recognizer.decode(stream);
            }

            // Report progress
            const progress = Math.round((i / samples.length) * 100);
            console.error(`Progress: ${progress}%`);

            // Check for endpoint
            if (recognizer.isEndpoint(stream)) {
                const result = recognizer.getResult(stream);
                const currentTime = i / sampleRate;

                if (result.text.trim()) {
                    let text = postProcessText(result.text.trim());
                    if (enableITN) text = applyITN(text);
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
        }

        // Get final result
        const finalResult = recognizer.getResult(stream);
        const totalDuration = samples.length / sampleRate;

        if (finalResult.text.trim()) {
            let text = postProcessText(finalResult.text.trim());
            if (enableITN) text = applyITN(text);
            segments.push({
                id: randomUUID(),
                text: text,
                start: segmentStartTime,
                end: totalDuration,
                isFinal: true,
            });
        }

        // Output segments as JSON array
        console.log(JSON.stringify(segments, null, 2));

    } catch (error) {
        console.error(JSON.stringify({ error: error.message }));
        process.exit(1);
    }
}

// Process audio file (Offline Mode)
async function processBatchOffline(recognizer, filePath, ffmpegPath, sampleRate, enableITN) {
    if (!existsSync(filePath)) {
        console.error(JSON.stringify({ error: `File not found: ${filePath}` }));
        process.exit(1);
    }

    try {
        console.error('Converting audio file (Offline)...');
        const pcmData = await convertToWav(filePath, ffmpegPath);

        const samples = new Float32Array(pcmData.length / 2);
        for (let i = 0; i < samples.length; i++) {
            const int16 = pcmData.readInt16LE(i * 2);
            samples[i] = int16 / 32768.0;
        }

        console.error(`Processing ${samples.length} samples with OfflineRecognizer...`);

        const stream = recognizer.createStream();
        stream.acceptWaveform({ samples: samples, sampleRate: sampleRate });

        recognizer.decode(stream);

        const result = recognizer.getResult(stream);
        const segments = [];

        // Offline recognizer might return full text.
        // Some offline models have timestamps in result.timestamps, result.tokens etc.
        // But basics: result.text

        if (result.text && result.text.trim()) {
            let text = postProcessText(result.text.trim());
            if (enableITN) text = applyITN(text);

            // Extract tokens and timestamps if available
            // standard sherpa-onnx-node offline recognizer result has .tokens and .timestamps
            const tokens = result.tokens || [];
            const timestamps = result.timestamps || [];

            segments.push({
                id: randomUUID(),
                text: text,
                start: 0,
                end: samples.length / sampleRate,
                isFinal: true,
                tokens: tokens,
                timestamps: timestamps
            });
        }

        console.log(JSON.stringify(segments, null, 2));

    } catch (error) {
        console.error(JSON.stringify({ error: error.message }));
        process.exit(1);
    }
}

// Ensure DYLD_LIBRARY_PATH is set on macOS
async function ensureDyldPath() {
    if (process.platform !== 'darwin') return false;
    if (process.env.SONA_DYLD_FIXED) return false;

    const arch = process.arch;
    const packageName = `sherpa-onnx-darwin-${arch}`;

    // Look for node_modules in likely locations
    const candidates = [
        join(__dirname, '../../node_modules'),
        join(__dirname, 'node_modules'),
        join(__dirname, '../node_modules'),
    ];

    let targetPath = null;
    for (const base of candidates) {
        const p = join(base, packageName);
        if (existsSync(p)) {
            targetPath = p;
            break;
        }
    }

    if (!targetPath) {
        return false;
    }

    const currentDyld = process.env.DYLD_LIBRARY_PATH || '';
    if (currentDyld.includes(targetPath)) {
        return false; // Already set
    }

    const newDyld = currentDyld
        ? `${targetPath}:${currentDyld}`
        : targetPath;

    console.error(`[Sidecar] Setting DYLD_LIBRARY_PATH to ${targetPath} and respawning...`);

    const newEnv = {
        ...process.env,
        DYLD_LIBRARY_PATH: newDyld,
        SONA_DYLD_FIXED: '1'
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

// Main entry point
async function main() {
    // Check for macOS DYLD fix
    if (await ensureDyldPath()) return;

    const options = parseArgs();

    if (!options.modelPath) {
        console.error(JSON.stringify({
            error: 'Model path is required. Use --model-path /path/to/model'
        }));
        process.exit(1);
    }

    const ffmpegPath = await getFFmpegPath();

    // For demo/testing without actual sherpa-onnx
    // TODO: Remove this mock mode in production
    const useMock = !existsSync(options.modelPath) || process.env.SONA_MOCK === '1';

    if (useMock) {
        console.error('Running in mock mode (sherpa-onnx model not found)');

        if (options.mode === 'batch' && options.file) {
            // Mock batch processing
            const mockSegments = [
                { id: randomUUID(), start: 0, end: 3.5, text: 'This is a mock transcript segment.', isFinal: true },
                { id: randomUUID(), start: 3.5, end: 7.2, text: 'Actual transcription will work when sherpa-onnx model is configured.', isFinal: true },
                { id: randomUUID(), start: 7.2, end: 10.0, text: 'Please set the model path in settings.', isFinal: true },
            ];
            console.log(JSON.stringify(mockSegments, null, 2));
        } else {
            // Mock stream processing
            console.log(JSON.stringify({
                id: randomUUID(),
                text: 'Mock streaming mode active',
                start: 0,
                end: 1,
                isFinal: true
            }));
        }

        process.exit(0);
    }

    try {
        const { recognizer, type, supportsInternalITN } = await createRecognizer(options.modelPath, options.enableITN);

        // Use JS ITN only if enabled AND model doesn't support it internally
        const useJSITN = options.enableITN && !supportsInternalITN;

        if (options.mode === 'batch') {
            if (!options.file) {
                console.error(JSON.stringify({ error: 'File path required for batch mode' }));
                process.exit(1);
            }
            if (type === 'offline') {
                // Offline models (SenseVoice) handle ITN internally if configured, so we pass false for JS ITN
                await processBatchOffline(recognizer, options.file, ffmpegPath, options.sampleRate, false);
            } else {
                await processBatch(recognizer, options.file, ffmpegPath, options.sampleRate, useJSITN);
            }
        } else {
            if (type === 'offline') {
                console.error(JSON.stringify({ error: 'Streaming mode not supported for this offline model (SenseVoice).' }));
                process.exit(1);
            }
            await processStream(recognizer, options.sampleRate, useJSITN);
        }
    } catch (error) {
        console.error(JSON.stringify({ error: error.message }));
        process.exit(1);
    }
}

main();
