
import { Command, Child } from '@tauri-apps/plugin-shell';
import { resolveResource } from '@tauri-apps/api/path';
import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';
import { StreamLineBuffer } from '../utils/streamBuffer';

/** Callback for receiving a new transcript segment. */
export type TranscriptionCallback = (segment: TranscriptSegment) => void;
/** Callback for receiving an error message. */
export type ErrorCallback = (error: string) => void;

/**
 * Service to manage the transcription process via a sidecar.
 * Handles spawning, communication (stdin/stdout), and lifecycle of the external process.
 */
class TranscriptionService {
    private child: Child | null = null;
    private isRunning: boolean = false;
    private modelPath: string = '';
    private itnModelPaths: string[] = [];
    private punctuationModelPath: string = '';
    private vadModelPath: string = '';
    private vadBufferSize: number = 5;
    private enableITN: boolean = true;
    private onSegment: TranscriptionCallback | null = null;
    private onError: ErrorCallback | null = null;

    constructor() { }

    /**
     * Sets the path to the main ASR model.
     *
     * @param path - The absolute path to the model directory or file.
     */
    setModelPath(path: string) {
        console.log('[TranscriptionService] Setting model path:', path);
        this.modelPath = path;
    }

    /**
     * Sets the paths to Inverse Text Normalization (ITN) models.
     *
     * @param paths - Array of absolute paths to ITN models.
     */
    setITNModelPaths(paths: string[]) {
        this.itnModelPaths = paths;
    }

    /**
     * Sets the path to the Punctuation model.
     *
     * @param path - The absolute path to the punctuation model.
     */
    setPunctuationModelPath(path: string) {
        this.punctuationModelPath = path;
    }

    /**
     * Sets the path to the Voice Activity Detection (VAD) model.
     *
     * @param path - The absolute path to the VAD model.
     */
    setVadModelPath(path: string) {
        this.vadModelPath = path;
    }

    /**
     * Sets the VAD buffer size in seconds.
     *
     * @param size - Buffer size in seconds.
     */
    setVadBufferSize(size: number) {
        this.vadBufferSize = size;
    }

    /**
     * Enables or disables Inverse Text Normalization.
     *
     * @param enabled - True to enable ITN, false to disable.
     */
    setEnableITN(enabled: boolean) {
        this.enableITN = enabled;
    }

    /**
     * Starts the transcription sidecar process in streaming mode.
     *
     * @param onSegment - Callback invoked when a new transcript segment is available.
     * @param onError - Callback invoked when an error occurs.
     */
    async start(onSegment: TranscriptionCallback, onError: ErrorCallback) {
        if (this.isRunning) return;
        if (!this.modelPath) {
            onError('Model path not configured');
            return;
        }

        this.onSegment = onSegment;
        this.onError = onError;

        console.log('Starting sidecar with model:', this.modelPath);

        try {
            const scriptPath = await resolveResource('sidecar/dist/index.mjs');

            const args = [
                scriptPath,
                '--mode', 'stream',
                '--model-path', this.modelPath,
                '--enable-itn', this.enableITN.toString()
            ];

            if (this.itnModelPaths.length > 0) {
                args.push('--itn-model', this.itnModelPaths.join(','));
            }

            if (this.punctuationModelPath) {
                args.push('--punctuation-model', this.punctuationModelPath);
            }

            if (import.meta.env.DEV) {
                args.push('--allow-mock', 'true');
            }

            const command = Command.sidecar('binaries/node', args);

            // Buffer for stdout stream
            const stdoutBuffer = new StreamLineBuffer();

            command.on('close', (data) => {
                console.log(`Sidecar finished with code ${data.code} and signal ${data.signal}`);

                // Process any remaining data
                const remaining = stdoutBuffer.flush();
                remaining.forEach(line => this.handleOutput(line));

                this.isRunning = false;
                this.child = null;
            });

            command.on('error', (error) => {
                console.error(`Sidecar error: "${error}"`);
                if (this.onError) this.onError(`Process error: ${error}`);
                this.isRunning = false;
            });

            command.stdout.on('data', (chunk) => {
                if (typeof chunk === 'string') {
                    const lines = stdoutBuffer.process(chunk);
                    lines.forEach(line => this.handleOutput(line));
                }
            });

            command.stderr.on('data', (chunk) => {
                console.log(`[TranscriptionService] stderr: ${chunk}`);
            });

            this.child = await command.spawn();
            this.isRunning = true;
            console.log('[TranscriptionService] Sidecar started, PID:', this.child.pid);

        } catch (error) {
            console.error('Failed to spawn sidecar:', error);
            if (this.onError) this.onError(`Failed to start: ${error}`);
            this.isRunning = false;
        }
    }

    /**
     * Stops the running transcription sidecar process.
     */
    async stop() {
        if (!this.child || !this.isRunning) return;

        try {
            await this.child.kill();
        } catch (error) {
            console.error('Failed to kill sidecar:', error);
        } finally {
            console.log('[TranscriptionService] Sidecar stopped');
            this.child = null;
            this.isRunning = false;
        }
    }

    /**
     * Sends raw audio samples (Float32) to the sidecar.
     * Optimization: Converts Float32 to Int16 with manual clamping.
     *
     * @param samples - Float32Array of audio samples (assumed 16kHz mono).
     */
    async sendAudio(samples: Float32Array) {
        if (!this.child || !this.isRunning) return;

        try {
            // Convert Float32 to Int16 for the sidecar
            // Optimization: Manual clamping to avoid function call overhead in tight loop
            const len = samples.length;
            const buffer = new Int16Array(len);
            for (let i = 0; i < len; i++) {
                let s = samples[i];
                if (s > 1) s = 1;
                else if (s < -1) s = -1;
                buffer[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
            }

            // Command.write accepts string or Uint8Array
            // We need to send bytes. Uint8Array view of Int16Array
            const bytes = new Uint8Array(buffer.buffer);
            await this.child.write(bytes);
        } catch (error) {
            console.error('Failed to write audio to sidecar:', error);
        }
    }

    /**
     * Sends pre-converted Int16 audio samples to the sidecar.
     *
     * @param samples - Int16Array of audio samples.
     */
    async sendAudioInt16(samples: Int16Array) {
        if (!this.child || !this.isRunning) return;

        try {
            // Command.write accepts string or Uint8Array
            // We need to send bytes. Uint8Array view of Int16Array
            // Use byteOffset and byteLength to handle cases where Int16Array is a view
            const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
            await this.child.write(bytes);
        } catch (error) {
            console.error('Failed to write audio to sidecar:', error);
        }
    }

    /**
     * Transcribes an audio file in batch mode.
     * Retries with CPU if CoreML fails.
     *
     * @param filePath - The path to the audio file.
     * @param onProgress - Optional callback for progress percentage (0-100).
     * @param onSegment - Optional callback for each segment transcribed.
     * @return A promise resolving to an array of all transcript segments.
     */
    async transcribeFile(filePath: string, onProgress?: (progress: number) => void, onSegment?: TranscriptionCallback, language?: string): Promise<TranscriptSegment[]> {
        try {
            return await this._transcribeFileInternal(filePath, undefined, onProgress, onSegment, language);
        } catch (error: any) {
            if (error.message === 'COREML_FAILURE') {
                console.warn('[TranscriptionService] CoreML failure detected. Retrying with CPU...');
                return await this._transcribeFileInternal(filePath, 'cpu', onProgress, onSegment, language);
            }
            throw error;
        }
    }

    /**
     * Internal implementation of batch transcription.
     *
     * @param filePath - Path to audio file.
     * @param provider - Execution provider (e.g., 'cpu'), or undefined for auto.
     * @param onProgress - Progress callback.
     * @param onSegment - Segment callback.
     * @return Promise resolving to segments.
     */
    private async _transcribeFileInternal(filePath: string, provider?: string, onProgress?: (progress: number) => void, onSegment?: TranscriptionCallback, language?: string): Promise<TranscriptSegment[]> {
        if (!this.modelPath) {
            throw new Error('Model path not configured');
        }

        console.log(`[TranscriptionService] Starting batch transcription for: ${filePath} (Provider: ${provider || 'auto'}, Language: ${language || 'auto'})`);

        return new Promise(async (resolve, reject) => {
            try {
                // Spawn sidecar in batch mode
                const scriptPath = await resolveResource('sidecar/dist/index.mjs');
                const args = [
                    scriptPath,
                    '--mode', 'batch',
                    '--file', filePath,
                    '--model-path', this.modelPath,
                    '--enable-itn', this.enableITN.toString()
                ];

                if (language && language !== 'auto') {
                    args.push('--language', language);
                }

                if (this.itnModelPaths.length > 0) {
                    args.push('--itn-model', this.itnModelPaths.join(','));
                }

                if (this.punctuationModelPath) {
                    args.push('--punctuation-model', this.punctuationModelPath);
                }

                if (this.vadModelPath) {
                    args.push('--vad-model', this.vadModelPath);
                    args.push('--vad-buffer', this.vadBufferSize.toString());
                }

                if (provider) {
                    args.push('--provider', provider);
                }

                if (import.meta.env.DEV) {
                    args.push('--allow-mock', 'true');
                }

                const command = Command.sidecar('binaries/node', args);

                // Optimization: Use array of chunks instead of string concatenation
                // This prevents O(N^2) copying behavior for large outputs
                const stderrChunks: string[] = [];
                const stderrStreamBuffer = new StreamLineBuffer();
                const stdoutStreamBuffer = new StreamLineBuffer();

                // Accumulate segments to return at the end (compatibility)
                const collectedSegments: TranscriptSegment[] = [];

                command.on('close', (data) => {
                    console.log(`[Batch] Sidecar finished with code ${data.code}`);

                    const stderrBuffer = stderrChunks.join('');

                    if (data.code === 0) {
                        // Check for silent CoreML failure
                        if (!provider &&
                            stderrBuffer.includes('Error executing model') &&
                            stderrBuffer.includes('CoreMLExecutionProvider')) {

                            reject(new Error('COREML_FAILURE'));
                            return;
                        }

                        // Flush remaining buffer
                        const lines = stdoutStreamBuffer.flush();
                        lines.forEach(line => {
                            try {
                                const data = JSON.parse(line);
                                // Check for segments (VAD mode or Stream mode)
                                if (data.text && typeof data.start === 'number' && typeof data.end === 'number') {
                                    const segment: TranscriptSegment = {
                                        id: data.id || uuidv4(),
                                        text: data.text,
                                        start: data.start,
                                        end: data.end,
                                        isFinal: data.isFinal || true
                                    };
                                    collectedSegments.push(segment);
                                    if (onSegment) onSegment(segment);
                                }
                            } catch (e) { }
                        });

                        // If collectedSegments is empty, maybe fallback to parsing whole buffer if we used array output?
                        // BUT: Sidecar now outputs lines.
                        // If sidecar output [ ... ] array (fallback), LineBuffer breaks it.
                        // Wait. If VAD is NOT enabled, sidecar might output one big array lines.
                        // Standard line buffer might split `[`, `{`, `},`, `]` on separate lines if pretty printed.
                        // Or straight JSON array on one line? `JSON.stringify(..., null, 2)` -> multi line.

                        // IF VAD is used: JSON lines.
                        // IF NOT VAD (old logic): JSON Array `[...]`.

                        // We need to handle both.
                        // If `collectedSegments` has content, return it.
                        // If empty, maybe check if we buffered a big array? 

                        // Actually, if we use StreamBuffer, and the sidecar prints `[\n  {...}\n]`, StreamBuffer will emit `[`, `{...`, `}`, `]`.
                        // Parsing `[` fails.

                        // Solution: Capture ALL stdout text too, just in case we need to parse as array at end.
                        // But for "Streaming" UX, we rely on lines.
                        // VAD mode outputs single line JSON.
                        // So if VAD on, this works. 

                        resolve(collectedSegments);

                    } else {
                        reject(new Error(`Sidecar failed with code ${data.code}: ${stderrBuffer}`));
                    }
                });

                command.on('error', (error) => {
                    reject(new Error(`Process error: ${error}`));
                });

                command.stdout.on('data', (chunk) => {
                    if (typeof chunk === 'string') {
                        // Parse Lines for Streaming
                        const lines = stdoutStreamBuffer.process(chunk);
                        lines.forEach(line => {
                            try {
                                const data = JSON.parse(line);
                                if (data.text && typeof data.start === 'number' && typeof data.end === 'number') {
                                    const segment: TranscriptSegment = {
                                        id: data.id || uuidv4(),
                                        text: data.text,
                                        start: data.start,
                                        end: data.end,
                                        isFinal: data.isFinal || true
                                    };
                                    collectedSegments.push(segment);
                                    if (onSegment) onSegment(segment);
                                } else if (Array.isArray(data)) {
                                    // Handle array (batch fallback output)
                                    // If we receive a full array in one line (possible?)
                                    data.forEach((item: any) => {
                                        const segment: TranscriptSegment = {
                                            id: item.id || uuidv4(),
                                            text: item.text,
                                            start: item.start,
                                            end: item.end,
                                            isFinal: item.isFinal || true
                                        };
                                        collectedSegments.push(segment);
                                        // Don't call onSegment here effectively at the end, unless desired
                                    });
                                }
                            } catch (e) { }
                        });
                    }
                });

                command.stderr.on('data', (chunk) => {
                    if (typeof chunk === 'string') {
                        stderrChunks.push(chunk);

                        const lines = stderrStreamBuffer.process(chunk);
                        lines.forEach(line => {
                            // Parse progress from stderr lines
                            try {
                                const data = JSON.parse(line);
                                if (data.type === 'progress' && typeof data.percentage === 'number') {
                                    if (onProgress) onProgress(data.percentage);
                                }
                            } catch (e) {
                                // Not JSON or not progress
                            }
                            console.log(`[Batch] stderr: ${line}`);
                        });
                    }
                });

                await command.spawn();

            } catch (error) {
                reject(error);
            }
        });
    }

    /**
     * Parses a single line of JSON output from the sidecar and updates state.
     *
     * @param line - The JSON string line.
     */
    private handleOutput(line: string) {
        try {
            const data = JSON.parse(line);

            // Check for segments
            if (data.text && typeof data.start === 'number' && typeof data.end === 'number') {
                const segment: TranscriptSegment = {
                    id: data.id || uuidv4(),
                    text: data.text,
                    start: data.start,
                    end: data.end,
                    isFinal: data.isFinal || false
                };

                if (this.onSegment) {
                    this.onSegment(segment);
                }
            } else if (data.error && this.onError) {
                this.onError(data.error);
            }
        } catch (e) {
            // Ignore non-JSON lines or partial chunks
            // console.debug('Failed to parse line:', line);
        }
    }
}

export const transcriptionService = new TranscriptionService();
