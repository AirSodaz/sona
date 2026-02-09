#!/usr/bin/env node
/**
 * Sona - Sherpa-onnx/ncnn Speech Recognition Sidecar
 * 
 * Supports two operational modes:
 * - Mode A (Stream): Reads PCM audio from stdin, outputs JSON lines
 * - Mode B (Batch): Processes audio file, outputs full segment array
 * 
 * Supports:
 * - sherpa-onnx-node: For CPU inference (ONNX models)
 */

import { spawn, execSync } from 'child_process';
import { randomUUID } from 'crypto';
import { createReadStream, existsSync, readdirSync, statSync, unlinkSync } from 'fs';
import os from 'os';
import { dirname, join } from 'path';
import { Readable, Transform } from 'stream';
import { fileURLToPath } from 'url';
import Seven from 'node-7z';

const __scriptFile = fileURLToPath(import.meta.url);
const __scriptDir = dirname(__scriptFile);

/**
 * Convert Int16 PCM buffer to Float32 audio samples.
 * @param {Buffer} buffer - Raw PCM data as Int16LE
 * @returns {Float32Array} Normalized audio samples
 */
function pcmInt16ToFloat32(buffer) {
    const samples = new Float32Array(buffer.length / 2);
    for (let i = 0; i < samples.length; i++) {
        samples[i] = buffer.readInt16LE(i * 2) / 32768.0;
    }
    return samples;
}

/**
 * Format transcript text with capitalization and punctuation.
 * @param {string} text - Raw text from recognizer
 * @param {object} [punctuation] - Optional punctuation model
 * @returns {string} Formatted text
 */
function formatTranscript(text, punctuation) {
    if (!text || !text.trim()) return '';
    let result = text.trim();

    // Fix all-caps output (naive heuristic)
    if (/[a-zA-Z]/.test(result) && result === result.toUpperCase()) {
        const lower = result.toLowerCase();
        result = lower.charAt(0).toUpperCase() + lower.slice(1);
    }

    if (punctuation) {
        result = punctuation.addPunct(result);
    }
    return result;
}

/**
 * Synthesize token durations from timestamps.
 * Since OnlineRecognizer only provides start timestamps, we calculate durations
 * as the difference between consecutive timestamps.
 * @param {number[]} timestamps - Array of absolute token start times
 * @param {number} segmentEndTime - End time of the segment
 * @returns {number[]} Array of token durations
 */
function synthesizeDurations(timestamps, segmentEndTime) {
    if (!timestamps || timestamps.length === 0) return [];
    return timestamps.map((t, i) => {
        const nextTime = timestamps[i + 1] ?? segmentEndTime;
        return nextTime - t;
    });
}

/**
 * Filter and validate ITN model paths.
 * @param {string} itnModel - Comma-separated paths
 * @returns {string|null} Valid paths joined by comma, or null if none valid
 */
function getValidItnPaths(itnModel) {
    if (!itnModel) return null;
    const paths = itnModel.split(',');
    const validPaths = paths.filter(p => existsSync(p.trim()));
    return validPaths.length > 0 ? validPaths.join(',') : null;
}

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
        numThreads: null,
        targetDir: null,
        itnModel: null,
        punctuationModel: null,
        vadBuffer: 5, // Default 5s
        language: '', // Default auto
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
            case '--vad-model':
                options.vadModel = args[++i];
                break;
            case '--vad-buffer':
                options.vadBuffer = parseFloat(args[++i]);
                break;
            case '--language':
                options.language = args[++i];
                break;
        }
    }

    return options;
}

/**
 * Get path to FFmpeg binary.
 * Checks local directory first, then system path, then static fallback.
 * @returns {Promise<string>} Path to ffmpeg executable
 */
async function getFFmpegPath() {
    const localBinary = join(__scriptDir, 'ffmpeg.exe');
    if (existsSync(localBinary)) return localBinary;

    const localBinaryNix = join(__scriptDir, 'ffmpeg');
    if (existsSync(localBinaryNix)) return localBinaryNix;

    const cwdBinary = join(process.cwd(), 'ffmpeg.exe');
    if (existsSync(cwdBinary)) return cwdBinary;

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

            const validItnPaths = getValidItnPaths(itnModel);
            if (validItnPaths) {
                console.error(`[Sidecar] Using ITN models (Offline): ${validItnPaths}`);
                config.ruleFsts = validItnPaths;
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

        const validItnPaths = getValidItnPaths(itnModel);
        if (validItnPaths) {
            console.error(`[Sidecar] Using ITN models: ${validItnPaths}`);
            config.ruleFsts = validItnPaths;
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
function findModelConfig(modelPath, enableITN, numThreads, language) {
    if (!existsSync(modelPath)) return null;

    try {
        const files = readdirSync(modelPath);
        const tokens = files.find(f => f === 'tokens.txt');

        if (!tokens) return null; // All models need tokens.txt

        // Check for ONNX files
        function findBestMatch(prefix) {
            const candidates = files.filter(f => f.includes(prefix) && f.endsWith('.onnx'));
            if (candidates.length === 0) return null;
            const int8 = candidates.find(f => f.includes('int8'));
            if (int8) return join(modelPath, int8);
            return join(modelPath, candidates[0]);
        }

        const encoder = findBestMatch('encoder');
        const decoder = findBestMatch('decoder');
        const joiner = findBestMatch('joiner');

        if (encoder && decoder) {
            const baseConfig = {
                tokens: join(modelPath, tokens),
                numThreads: numThreads,
                provider: 'cpu', // ONNX is CPU only now per requirement
                debug: false,
                language: '', // Placeholder, will be overwritten if passed separately? Or passed to findModelConfig?
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
                        language: language || '',
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





// Process Stream
async function processStream(recognizer, sampleRate, punctuation) {
    const stream = recognizer.createStream();
    let totalSamples = 0;
    let segmentStartTime = 0;
    let currentSegmentId = randomUUID();

    // Track token timestamps incrementally
    // Each entry: { token, endTime } - start time is derived from previous token's endTime
    let trackedTokens = [];
    let lastTokenCount = 0;

    process.stdin.on('data', (chunk) => {
        if (chunk.toString() === '__EOS__') {
            process.stdin.emit('end');
            return;
        }

        const samples = pcmInt16ToFloat32(chunk);
        stream.acceptWaveform({ samples, sampleRate });
        totalSamples += samples.length;

        while (recognizer.isReady(stream)) {
            recognizer.decode(stream);
        }

        const result = recognizer.getResult(stream);
        const currentTime = totalSamples / sampleRate;
        const currentTokens = result.tokens || [];

        // Track new tokens - if token count increased, record timestamps for new tokens
        if (currentTokens.length > lastTokenCount) {
            for (let i = lastTokenCount; i < currentTokens.length; i++) {
                trackedTokens.push({
                    token: currentTokens[i],
                    endTime: currentTime
                });
            }
            lastTokenCount = currentTokens.length;
        }

        // Partial result
        if (result.text.trim()) {
            const text = formatTranscript(result.text); // No punctuation for partials
            console.log(JSON.stringify({
                id: currentSegmentId,
                text,
                start: segmentStartTime,
                end: currentTime,
                isFinal: false,
            }));
        }

        // Endpoint detected
        if (recognizer.isEndpoint(stream)) {
            const finalResult = recognizer.getResult(stream);

            // Update tracked tokens with any remaining new tokens
            const finalTokens = finalResult.tokens || [];
            for (let i = lastTokenCount; i < finalTokens.length; i++) {
                trackedTokens.push({
                    token: finalTokens[i],
                    endTime: currentTime
                });
            }

            if (finalResult.text.trim()) {
                const text = formatTranscript(finalResult.text, punctuation);

                // Build timestamps and durations from tracked tokens
                const timestamps = [];
                const durations = [];
                for (let i = 0; i < trackedTokens.length; i++) {
                    const startTime = i === 0 ? segmentStartTime : trackedTokens[i - 1].endTime;
                    const endTime = trackedTokens[i].endTime;
                    timestamps.push(startTime);
                    durations.push(endTime - startTime);
                }

                console.log(JSON.stringify({
                    id: currentSegmentId,
                    text,
                    start: segmentStartTime,
                    end: currentTime,
                    isFinal: true,
                    tokens: trackedTokens.map(t => t.token),
                    timestamps,
                    durations
                }));
            }

            // Reset for next segment
            recognizer.reset(stream);
            segmentStartTime = currentTime;
            currentSegmentId = randomUUID();
            trackedTokens = [];
            lastTokenCount = 0;
        }
    });

    process.stdin.on('end', () => {
        const finalResult = recognizer.getResult(stream);
        const currentTime = totalSamples / sampleRate;

        // Update tracked tokens with any remaining new tokens
        const finalTokens = finalResult.tokens || [];
        for (let i = lastTokenCount; i < finalTokens.length; i++) {
            trackedTokens.push({
                token: finalTokens[i],
                endTime: currentTime
            });
        }

        if (finalResult.text.trim()) {
            const text = formatTranscript(finalResult.text, punctuation);

            // Build timestamps and durations from tracked tokens
            const timestamps = [];
            const durations = [];
            for (let i = 0; i < trackedTokens.length; i++) {
                const startTime = i === 0 ? segmentStartTime : trackedTokens[i - 1].endTime;
                const endTime = trackedTokens[i].endTime;
                timestamps.push(startTime);
                durations.push(endTime - startTime);
            }

            console.log(JSON.stringify({
                id: currentSegmentId,
                text,
                start: segmentStartTime,
                end: currentTime,
                isFinal: true,
                tokens: trackedTokens.map(t => t.token),
                timestamps,
                durations
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
        const samples = pcmInt16ToFloat32(pcmData);

        console.error(`Processing ${samples.length} samples...`);
        const stream = recognizer.createStream();
        const segments = [];
        let segmentStartTime = 0;
        const chunkSize = sampleRate * 0.5;

        for (let i = 0; i < samples.length; i += chunkSize) {
            const chunk = samples.slice(i, Math.min(i + chunkSize, samples.length));

            stream.acceptWaveform({ samples: chunk, sampleRate });

            while (recognizer.isReady(stream)) recognizer.decode(stream);

            if (recognizer.isEndpoint(stream)) {
                const result = recognizer.getResult(stream);
                const currentTime = i / sampleRate;

                if (result.text.trim()) {
                    const text = formatTranscript(result.text, punctuation);
                    segments.push({
                        id: randomUUID(),
                        text,
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
            const text = formatTranscript(finalResult.text, punctuation);
            segments.push({
                id: randomUUID(),
                text,
                start: segmentStartTime,
                end: totalDuration,
                isFinal: true,
            });
        }
        console.log(JSON.stringify(segments, null, 2));

    } finally {
        // Completed batch processing
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
                        unlinkSync(tarPath);
                    } catch (e) { /* Ignore deletion errors */ }

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




/**
 * Generator that yields audio segments using VAD.
 * @param {Float32Array} samples 
 * @param {number} sampleRate 
 * @param {object} vad - Initialized VAD instance
 * @param {number} vadBufferSeconds 
 */
async function* yieldVADSegments(samples, sampleRate, vad, vadBufferSeconds = 5) {
    const windowSize = Math.floor(sampleRate * 0.03); // 30ms for VAD
    const ringBufferSize = Math.ceil(vadBufferSeconds / 0.03);
    const ringBuffer = [];

    let currentSegmentSamples = [];
    let currentSegmentLength = 0;
    let processedSamples = 0;

    for (let i = 0; i < samples.length; i += windowSize) {
        const end = Math.min(i + windowSize, samples.length);
        const chunk = samples.slice(i, end);
        processedSamples = end;

        vad.acceptWaveform(chunk);

        if (vad.isDetected()) {
            // Speech Start
            if (currentSegmentLength === 0) {
                // Prepend ring buffer
                for (const bufChunk of ringBuffer) {
                    currentSegmentSamples.push(bufChunk);
                    currentSegmentLength += bufChunk.length;
                }
                ringBuffer.length = 0;
            }
            currentSegmentSamples.push(chunk);
            currentSegmentLength += chunk.length;
        } else {
            // Silence
            if (currentSegmentLength > 0) {
                // Speech End -> Yield Segment
                const totalSamples = new Float32Array(currentSegmentLength);
                let offset = 0;
                for (const c of currentSegmentSamples) {
                    totalSamples.set(c, offset);
                    offset += c.length;
                }

                yield {
                    type: 'segment',
                    samples: totalSamples,
                    endIndex: processedSamples,
                    duration: currentSegmentLength / sampleRate
                };

                currentSegmentSamples = [];
                currentSegmentLength = 0;
            }

            ringBuffer.push(chunk);
            if (ringBuffer.length > ringBufferSize) {
                ringBuffer.shift();
            }
        }

        // Progress
        if (i % sampleRate < windowSize) {
            const progress = Math.min(100, Math.round((i / samples.length) * 100));
            yield { type: 'progress', percentage: progress };
        }
    }

    // Flush remaining
    if (currentSegmentLength > 0) {
        const totalSamples = new Float32Array(currentSegmentLength);
        let offset = 0;
        for (const c of currentSegmentSamples) {
            totalSamples.set(c, offset);
            offset += c.length;
        }
        yield {
            type: 'segment',
            samples: totalSamples,
            endIndex: samples.length,
            duration: currentSegmentLength / sampleRate
        };
    }
}

/**
 * Generator that yields fixed audio chunks as fallback.
 * @param {Float32Array} samples
 * @param {number} sampleRate
 * @param {number} chunkDuration
 */
async function* yieldFixedChunks(samples, sampleRate, chunkDuration = 30) {
    const chunkSize = Math.floor(sampleRate * chunkDuration);
    for (let i = 0; i < samples.length; i += chunkSize) {
        const end = Math.min(i + chunkSize, samples.length);
        const chunk = samples.slice(i, end);

        yield {
            type: 'segment',
            samples: chunk,
            endIndex: end
        };

        const progress = Math.min(100, Math.round((end / samples.length) * 100));
        yield { type: 'progress', percentage: progress };
    }
}

async function processBatchOffline(recognizer, filePath, ffmpegPath, sampleRate, punctuation, vadModelPath, options) {
    if (!existsSync(filePath)) throw new Error(`File not found: ${filePath}`);

    try {
        console.error('Converting audio file (Offline VAD)...');
        const pcmData = await convertToWav(filePath, ffmpegPath);
        const samples = pcmInt16ToFloat32(pcmData);

        let vad = null;
        if (vadModelPath && existsSync(vadModelPath)) {
            const sherpa = await import('sherpa-onnx-node');
            const vadConfig = {
                sileroVad: {
                    model: vadModelPath,
                    threshold: 0.35,
                    minSilenceDuration: 1.0,
                    minSpeechDuration: 0.25,
                },
                sampleRate,
                debug: false,
                numThreads: 1,
            };
            vad = new sherpa.default.Vad(vadConfig, 60);
            console.error(`[Sidecar] VAD initialized with model: ${vadModelPath}`);
        } else {
            console.error('[Sidecar] No VAD model provided or found. Falling back to simple chunking.');
        }

        console.error(`Processing ${samples.length} samples...`);
        console.error(JSON.stringify({ type: 'progress', percentage: 0 }));

        const generator = vad
            ? yieldVADSegments(samples, sampleRate, vad, options.vadBuffer)
            : yieldFixedChunks(samples, sampleRate);

        for await (const item of generator) {
            if (item.type === 'progress') {
                console.error(JSON.stringify(item));
                continue;
            }

            if (item.type === 'segment') {
                const stream = recognizer.createStream();
                stream.acceptWaveform({ samples: item.samples, sampleRate });
                recognizer.decode(stream);
                const result = recognizer.getResult(stream);

                if (result && result.text && result.text.trim()) {
                    const text = formatTranscript(result.text, punctuation);
                    const endTime = item.endIndex / sampleRate;
                    // Approximating start time based on duration (not perfect but matches original logic intent)
                    // Original logic: startTime = Math.max(0, endTime - (currentSegmentLength / sampleRate));
                    // Here we yielded currentSegmentLength but didn't pass it back precisely as seconds.
                    // Wait, item.duration is available from VAD generator.
                    // For fixed chunks, we can calc from samples length.
                    const durationStr = item.duration || (item.samples.length / sampleRate);
                    const startTime = Math.max(0, endTime - durationStr);

                    const output = {
                        id: randomUUID(),
                        text,
                        start: startTime,
                        end: endTime,
                        isFinal: true,
                        isFinal: true,
                        tokens: result.tokens || [],
                        timestamps: (result.timestamps || []).map(t => t + startTime)
                    };
                    console.log(JSON.stringify(output));
                }
            }
        }

    } finally {
        // Cleanup if needed
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
        const found = findModelConfig(options.modelPath, options.enableITN, numThreads, options.language);
        if (!found) throw new Error('Could not find valid model configuration (ONNX)');

        const { recognizer, type } = await createOnnxRecognizer(
            found.config, options.enableITN, numThreads, options.itnModel
        );

        const punctuation = await createPunctuation(options.punctuationModel);

        if (options.mode === 'batch') {
            if (!options.file) throw new Error('File path required for batch mode');
            if (type === 'offline') {
                await processBatchOffline(recognizer, options.file, ffmpegPath, options.sampleRate, punctuation, options.vadModel, options);
            } else {
                await processBatch(recognizer, options.file, ffmpegPath, options.sampleRate, punctuation);
            }
        } else {
            if (type === 'offline') {
                console.error(JSON.stringify({ error: 'Streaming mode not supported for offline model.' }));
                process.exit(1);
            }

            await processStream(recognizer, options.sampleRate, punctuation);
        }

    } catch (error) {
        console.error(JSON.stringify({ error: error.message || String(error) }));
        process.exit(1);
    }
}

main();
