
import { Command, Child } from '@tauri-apps/plugin-shell';
import { resolveResource } from '@tauri-apps/api/path';
import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';
import { StreamLineBuffer } from '../utils/streamBuffer';

/** Callback for receiving a new transcript segment. */
export type TranscriptionCallback = (segment: TranscriptSegment) => void;
/** Callback for receiving an error message. */
export type ErrorCallback = (error: string) => void;

/** Result from CTC forced alignment. */
export interface AlignmentResult {
    /** List of tokens from CTC recognition. */
    tokens: string[];
    /** Absolute start timestamps for each token. */
    timestamps: number[];
    /** Duration of each token. */
    durations: number[];
    /** Raw text from CTC recognition (may differ from user-edited text). */
    ctcText: string;
}

/** Configuration used to spawn the sidecar process. */
interface ServiceConfig {
    modelPath: string;
    itnModelPaths: string[];
    punctuationModelPath: string;
    vadModelPath: string;
    vadBufferSize: number;
    enableITN: boolean;
}

/**
 * Service to manage the transcription process via a sidecar.
 *
 * Handles spawning, communication (stdin/stdout), and lifecycle of the external process.
 */
class TranscriptionService {
    private child: Child | null = null;
    /** Indicates if the sidecar process is currently running. */
    private isRunning: boolean = false;
    /** Path to the main ASR model. */
    private modelPath: string = '';
    /** List of paths to Inverse Text Normalization (ITN) models. */
    private itnModelPaths: string[] = [];
    /** Path to the punctuation model. */
    private punctuationModelPath: string = '';
    /** Path to the Voice Activity Detection (VAD) model. */
    private vadModelPath: string = '';
    /** Path to the CTC model. */
    private ctcModelPath: string = '';
    /** Path to the source audio file for alignment. */
    private sourceFilePath: string = '';
    /** Buffer size for VAD in seconds. */
    private vadBufferSize: number = 5;
    /** Whether to enable Inverse Text Normalization. */
    private enableITN: boolean = true;
    /** Callback for new transcript segments. */
    private onSegment: TranscriptionCallback | null = null;
    /** Callback for error reporting. */
    private onError: ErrorCallback | null = null;
    /** Promise to track active spawning to prevent race conditions. */
    private spawningPromise: Promise<void> | null = null;
    /** Configuration of the currently running process. */
    private runningConfig: ServiceConfig | null = null;

    /**
     * Initializes a new instance of the TranscriptionService.
     */
    constructor() { }

    /**
     * Sets the path to the main ASR model.
     *
     * @param path The absolute path to the model file or directory.
     */
    setModelPath(path: string): void {
        console.log('[TranscriptionService] Setting model path:', path);
        this.modelPath = path;
    }

    /**
     * Sets the paths for Inverse Text Normalization (ITN) models.
     *
     * @param paths A list of absolute paths to ITN models.
     */
    setITNModelPaths(paths: string[]): void {
        this.itnModelPaths = paths;
    }

    /**
     * Sets the path to the Punctuation model.
     *
     * @param path The absolute path to the punctuation model.
     */
    setPunctuationModelPath(path: string): void {
        this.punctuationModelPath = path;
    }

    /**
     * Sets the path to the Voice Activity Detection (VAD) model.
     *
     * @param path The absolute path to the VAD model.
     */
    setVadModelPath(path: string): void {
        this.vadModelPath = path;
    }

    /**
     * Sets the path to the CTC model.
     *
     * @param path The absolute path to the CTC model.
     */
    setCtcModelPath(path: string): void {
        this.ctcModelPath = path;
    }

    /**
     * Sets the path to the source audio file for alignment.
     *
     * @param path The absolute path to the audio file.
     */
    setSourceFilePath(path: string): void {
        this.sourceFilePath = path;
    }

    /**
     * Sets the VAD buffer size.
     *
     * @param size The buffer size in seconds.
     */
    setVadBufferSize(size: number): void {
        this.vadBufferSize = size;
    }

    /**
     * Enables or disables Inverse Text Normalization (ITN).
     *
     * @param enabled True to enable ITN, or false to disable it.
     */
    setEnableITN(enabled: boolean): void {
        this.enableITN = enabled;
    }

    /**
     * Starts the transcription process in streaming mode.
     *
     * @param onSegment A callback for when a new transcript segment is ready.
     * @param onError A callback for when an error occurs.
     */
    async prepare(): Promise<void> {
        // If running, check if config changed.
        if (this.isRunning) {
            if (this._isConfigMatch()) {
                return;
            }
            console.log('[TranscriptionService] Configuration changed, restarting sidecar...');
            await this.stop();
        }

        if (!this.modelPath) {
            console.warn('[TranscriptionService] Model path not configured, cannot prepare sidecar');
            return;
        }

        console.log('[TranscriptionService] Pre-spawning sidecar...');
        return this._spawnSidecar();
    }

    async start(onSegment: TranscriptionCallback, onError: ErrorCallback): Promise<void> {
        this.onSegment = onSegment;
        this.onError = onError;

        // If running, check if config changed.
        if (this.isRunning) {
            if (this._isConfigMatch()) {
                console.log('[TranscriptionService] Service already running with matching config, ready for audio');
                return;
            }
            console.log('[TranscriptionService] Configuration changed, restarting sidecar for start...');
            await this.stop();
        }

        if (!this.modelPath) {
            onError('Model path not configured');
            return;
        }

        await this._spawnSidecar();
    }

    private async _spawnSidecar(): Promise<void> {
        if (this.spawningPromise) {
            return this.spawningPromise;
        }

        this.spawningPromise = (async () => {
            console.log('Starting sidecar with model:', this.modelPath);

            // Capture the config we are about to use
            const configToUse: ServiceConfig = {
                modelPath: this.modelPath,
                itnModelPaths: [...this.itnModelPaths],
                punctuationModelPath: this.punctuationModelPath,
                vadModelPath: this.vadModelPath,
                vadBufferSize: this.vadBufferSize,
                enableITN: this.enableITN
            };

            try {
                const scriptPath = await resolveResource('sidecar/dist/index.mjs');

                const commonArgs = this._getCommonArgs();
                const args = [
                    scriptPath,
                    '--mode', 'stream',
                    ...commonArgs
                ];

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
                    const lines = stdoutBuffer.process(chunk);
                    lines.forEach(line => this.handleOutput(line));
                });

                command.stderr.on('data', (chunk) => {
                    console.log(`[TranscriptionService] stderr: ${chunk}`);
                });

                this.child = await command.spawn();
                this.isRunning = true;
                this.runningConfig = configToUse;
                console.log('[TranscriptionService] Sidecar started, PID:', this.child.pid);

            } catch (error) {
                console.error('Failed to spawn sidecar:', error);
                if (this.onError) this.onError(`Failed to start: ${error}`);
                this.isRunning = false;
                this.runningConfig = null;
                throw error;
            }
        })();

        try {
            await this.spawningPromise;
        } finally {
            this.spawningPromise = null;
        }
    }

    /**
     * Checks if the current configuration matches the running configuration.
     */
    private _isConfigMatch(): boolean {
        if (!this.runningConfig) return false;

        if (this.modelPath !== this.runningConfig.modelPath) return false;
        if (this.enableITN !== this.runningConfig.enableITN) return false;
        if (this.punctuationModelPath !== this.runningConfig.punctuationModelPath) return false;
        if (this.vadModelPath !== this.runningConfig.vadModelPath) return false;
        if (this.vadBufferSize !== this.runningConfig.vadBufferSize) return false;

        // Compare ITN model paths (array)
        if (this.itnModelPaths.length !== this.runningConfig.itnModelPaths.length) return false;
        // Assuming order matters as it affects rule application order
        for (let i = 0; i < this.itnModelPaths.length; i++) {
            if (this.itnModelPaths[i] !== this.runningConfig.itnModelPaths[i]) return false;
        }

        return true;
    }

    /**
     * Stops the transcription process.
     */
    async stop(): Promise<void> {
        if (!this.child || !this.isRunning) return;

        try {
            await this.child.kill();
        } catch (error) {
            console.error('Failed to kill sidecar:', error);
        } finally {
            console.log('[TranscriptionService] Sidecar stopped');
            this.child = null;
            this.isRunning = false;
            this.runningConfig = null;
        }
    }


    /**
     * Stops the transcription gracefully by sending an end-of-stream signal.
     *
     * This allows the model to finish the last segment and add punctuation.
     */
    async softStop(): Promise<void> {
        if (!this.child || !this.isRunning) return;

        try {
            console.log('[TranscriptionService] Sending __RESET__ command...');
            // Send Reset instead of EOS to keep process alive
            await this.child.write('__RESET__');

            // We don't wait for isRunning to become false anymore.
            // We could wait for a "reset" confirmation from the sidecar if we wanted strict sync,
            // but for now, sending the command is enough to trigger the flush and reset.
            // The sidecar prints {"reset": true} when done. 
            // We can optionally wait for that in handleOutput if we needed to block here.

            // To be safe, wait a small amount of time for flush
            await new Promise(r => setTimeout(r, 200));

        } catch (error) {
            console.error('Failed to soft stop sidecar:', error);
            // Don't kill process here, just log error. 
            // If it's truly stuck, the next start might fail or user can restart app.
        }
    }

    /**
     * Completely terminates the sidecar process.
     */
    async terminate(): Promise<void> {
        await this.stop();
    }

    /**
     * Sends audio samples to the transcription process.
     *
     * @param samples An array of Int16 audio samples.
     */
    async sendAudioInt16(samples: Int16Array): Promise<void> {
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
     * Transcribes an audio file.
     *
     * If CoreML fails, this method tries again using the CPU.
     *
     * @param filePath The absolute path to the audio file.
     * @param onProgress An optional callback for progress (0-100).
     * @param onSegment An optional callback for each transcribed segment.
     * @param language The language code (e.g., 'en', 'zh'). Defaults to 'auto'.
     * @param saveToPath Optional path to save the processed audio file (WAV format).
     * @return A promise that resolves to a list of all transcript segments.
     */
    async transcribeFile(filePath: string, onProgress?: (progress: number) => void, onSegment?: TranscriptionCallback, language?: string, saveToPath?: string): Promise<TranscriptSegment[]> {
        try {
            return await this._transcribeFileInternal(filePath, undefined, onProgress, onSegment, language, saveToPath);
        } catch (error: any) {
            if (error.message === 'COREML_FAILURE') {
                console.warn('[TranscriptionService] CoreML failure detected. Retrying with CPU...');
                return await this._transcribeFileInternal(filePath, 'cpu', onProgress, onSegment, language, saveToPath);
            }
            throw error;
        }
    }

    /**
     * Transcribes a file using a specific execution provider.
     *
     * @param filePath The absolute path to the audio file.
     * @param provider The execution provider (e.g., 'cpu'). If undefined, it uses 'auto'.
     * @param onProgress A callback for progress updates.
     * @param onSegment A callback for new segments.
     * @param language The language code.
     * @param saveToPath Optional path to save the processed audio file (WAV format).
     * @return A promise that resolves to the list of segments.
     */
    private async _transcribeFileInternal(filePath: string, provider?: string, onProgress?: (progress: number) => void, onSegment?: TranscriptionCallback, language?: string, saveToPath?: string): Promise<TranscriptSegment[]> {
        if (!this.modelPath) {
            throw new Error('Model path not configured');
        }

        console.log(`[TranscriptionService] Starting batch transcription for: ${filePath} (Provider: ${provider || 'auto'}, Language: ${language || 'auto'})`);

        // Spawn sidecar in batch mode
        const scriptPath = await resolveResource('sidecar/dist/index.mjs');
        const commonArgs = this._getCommonArgs();

        const args = [
            scriptPath,
            '--mode', 'batch',
            '--file', filePath,
            ...commonArgs
        ];

        if (language && language !== 'auto') {
            args.push('--language', language);
        }

        if (saveToPath) {
            args.push('--save-wav', saveToPath);
        }

        if (this.vadModelPath) {
            args.push('--vad-model', this.vadModelPath);
            args.push('--vad-buffer', this.vadBufferSize.toString());
        }

        if (provider) {
            args.push('--provider', provider);
        }

        const command = Command.sidecar('binaries/node', args);

        // Optimization: Use array of chunks instead of string concatenation
        // This prevents O(N^2) copying behavior for large outputs
        const stderrChunks: string[] = [];
        const stderrStreamBuffer = new StreamLineBuffer();
        const stdoutStreamBuffer = new StreamLineBuffer();

        // Accumulate segments to return at the end (compatibility)
        const collectedSegments: TranscriptSegment[] = [];

        // Wrap execution in a promise to await completion
        return new Promise<TranscriptSegment[]>(async (resolve, reject) => {
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
                            if (data.text && typeof data.start === 'number' && typeof data.end === 'number') {
                                const segment: TranscriptSegment = {
                                    id: data.id || uuidv4(),
                                    text: data.text,
                                    start: data.start,
                                    end: data.end,
                                    isFinal: data.isFinal || true,
                                    tokens: data.tokens,
                                    timestamps: data.timestamps
                                };
                                collectedSegments.push(segment);
                                if (onSegment) onSegment(segment);
                            }
                        } catch (e) { }
                    });

                    resolve(collectedSegments);

                } else {
                    reject(new Error(`Sidecar failed with code ${data.code}: ${stderrBuffer}`));
                }
            });

            command.on('error', (error) => {
                reject(new Error(`Process error: ${error}`));
            });

            command.stdout.on('data', (chunk) => {
                const lines = stdoutStreamBuffer.process(chunk);
                lines.forEach(line => {
                    try {
                        const data = JSON.parse(line);
                        if (Array.isArray(data)) {
                            data.forEach((item: any) => {
                                const segment = this._createSegment(item, true);
                                if (segment) {
                                    collectedSegments.push(segment);
                                }
                            });
                        } else {
                            const segment = this._createSegment(data, true);
                            if (segment) {
                                collectedSegments.push(segment);
                                if (onSegment) onSegment(segment);
                            }
                        }
                    } catch (e) {
                    }
                });
            });

            command.stderr.on('data', (chunk) => {
                stderrChunks.push(chunk);
                const lines = stderrStreamBuffer.process(chunk);
                lines.forEach(line => {
                    try {
                        const data = JSON.parse(line);
                        if (data.type === 'progress' && typeof data.percentage === 'number') {
                            if (onProgress) onProgress(data.percentage);
                        }
                    } catch (e) {
                    }
                    console.log(`[Batch] stderr: ${line}`);
                });
            });

            try {
                await command.spawn();
            } catch (error) {
                reject(error);
            }
        });
    }

    /**
     * Parses a line of JSON output and updates the state.
     *
     * @param line A string containing the JSON output.
     */
    private handleOutput(line: string): void {
        try {
            const data = JSON.parse(line);

            // Check for segments
            const segment = this._createSegment(data, false);
            if (segment) {
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

    /**
     * Generates the common arguments for the sidecar process.
     * 
     * @returns An array of argument strings.
     */
    private _getCommonArgs(): string[] {
        const args = [
            '--model-path', this.modelPath,
            '--enable-itn', this.enableITN.toString()
        ];

        if (this.enableITN && this.itnModelPaths.length > 0) {
            args.push('--itn-model', this.itnModelPaths.join(','));
        }

        if (this.punctuationModelPath) {
            args.push('--punctuation-model', this.punctuationModelPath);
        }

        if (import.meta.env.DEV) {
            args.push('--allow-mock', 'true');
        }

        if (this.vadModelPath) {
            args.push('--vad-model', this.vadModelPath);
            args.push('--vad-buffer', this.vadBufferSize.toString());
        }

        return args;
    }

    /**
     * Emits a segment as if it came from the sidecar.
     * Useful for testing.
     * @internal
     */
    emitSegment(segment: TranscriptSegment): void {
        if (this.onSegment) {
            this.onSegment(segment);
        }
    }

    /**
     * Runs CTC forced alignment on a segment's audio slice.
     *
     * Spawns the sidecar in align mode to re-recognize the audio and produce
     * fresh tokens/timestamps/durations. Returns null on any failure.
     *
     * @param segment The segment to align (uses start/end times).
     * @param sourceFilePath Optional override for the audio file path.
     * @return The alignment result, or null if alignment is unavailable or fails.
     */
    async alignSegment(segment: TranscriptSegment, sourceFilePath?: string): Promise<AlignmentResult | null> {
        const filePath = sourceFilePath || this.sourceFilePath;
        if (!this.ctcModelPath) {
            console.log('[TranscriptionService] No CTC model configured, skipping alignment');
            return null;
        }
        if (!filePath) {
            console.log('[TranscriptionService] No source file path, skipping alignment');
            return null;
        }

        console.log(`[TranscriptionService] Aligning segment ${segment.id} (${segment.start}-${segment.end})`);

        try {
            const scriptPath = await resolveResource('sidecar/dist/index.mjs');
            const args = [
                scriptPath,
                '--mode', 'align',
                '--file', filePath,
                '--ctc-model', this.ctcModelPath,
                '--start-time', segment.start.toString(),
                '--end-time', segment.end.toString(),
            ];

            const command = Command.sidecar('binaries/node', args);
            const stdoutBuffer = new StreamLineBuffer();

            return new Promise<AlignmentResult | null>((resolve) => {
                let result: AlignmentResult | null = null;

                command.stdout.on('data', (chunk) => {
                    const lines = stdoutBuffer.process(chunk);
                    for (const line of lines) {
                        try {
                            const data = JSON.parse(line);
                            if (data.tokens && Array.isArray(data.tokens)) {
                                result = {
                                    tokens: data.tokens,
                                    timestamps: data.timestamps || [],
                                    durations: data.durations || [],
                                    ctcText: data.ctcText || '',
                                };
                            }
                        } catch (e) { /* ignore non-JSON */
                        }
                    }
                });

                command.stderr.on('data', (chunk) => {
                    console.log(`[Align] stderr: ${chunk}`);
                });

                command.on('close', (data) => {
                    // Flush remaining buffer
                    const remaining = stdoutBuffer.flush();
                    for (const line of remaining) {
                        try {
                            const data = JSON.parse(line);
                            if (data.tokens && Array.isArray(data.tokens)) {
                                result = {
                                    tokens: data.tokens,
                                    timestamps: data.timestamps || [],
                                    durations: data.durations || [],
                                    ctcText: data.ctcText || '',
                                };
                            }
                        } catch (e) { /* ignore non-JSON */ }
                    }

                    if (data.code === 0 && result && result.tokens.length > 0) {
                        resolve(result);
                    } else {
                        console.warn(`[TranscriptionService] Alignment failed (code ${data.code})`);
                        resolve(null);
                    }
                });

                command.on('error', (error) => {
                    console.error(`[TranscriptionService] Alignment process error: ${error}`);
                    resolve(null);
                });

                command.spawn().catch((error) => {
                    console.error(`[TranscriptionService] Failed to spawn alignment sidecar: ${error}`);
                    resolve(null);
                });
            });
        } catch (error) {
            console.error('[TranscriptionService] Alignment error:', error);
            return null;
        }
    }

    /**
     * Creates a TranscriptSegment from raw data.
     * 
     * @param data The raw data object.
     * @param defaultIsFinal The default value for isFinal if not present in data.
     * @returns A TranscriptSegment or null if data is invalid.
     */
    private _createSegment(data: any, defaultIsFinal: boolean): TranscriptSegment | null {
        if (data && typeof data.text === 'string' && typeof data.start === 'number' && typeof data.end === 'number') {
            return {
                id: data.id || uuidv4(),
                text: data.text,
                start: data.start,
                end: data.end,
                isFinal: data.isFinal ?? defaultIsFinal,
                tokens: data.tokens,
                timestamps: data.timestamps,
                durations: data.durations
            };
        }
        return null;
    }
}

export const transcriptionService = new TranscriptionService();
