import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { TranscriptSegment } from '../types/transcript';
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

/** Configuration used to start the backend recognizer. */
interface ServiceConfig {
    modelPath: string;
    itnModelPaths: string[];
    punctuationModelPath: string;
    vadModelPath: string;
    vadBufferSize: number;
    enableITN: boolean;
    language: string;
}

/**
 * Service to manage the transcription process via the Rust backend.
 *
 * Handles starting, communicating, and lifecycle of the recognizer instance.
 */
export class TranscriptionService {
    /** Indicates if the recognizer is currently running. */
    private isRunning: boolean = false;
    /** Function to stop listening for recognizer events. */
    private unlistenOutput: UnlistenFn | null = null;
    /** Path to the main ASR model. */
    private modelPath: string = '';
    /** List of paths to Inverse Text Normalization (ITN) models. */
    private itnModelPaths: string[] = [];
    /** Path to the punctuation model. */
    private punctuationModelPath: string = '';
    /** Path to the Voice Activity Detection (VAD) model. */
    private vadModelPath: string = '';
    /** Buffer size for VAD in seconds. */
    private vadBufferSize: number = 5;
    /** Whether to enable Inverse Text Normalization. */
    private enableITN: boolean = true;
    /** Callback for new transcript segments. */
    private onSegment: TranscriptionCallback | null = null;
    /** Callback for error reporting. */
    private onError: ErrorCallback | null = null;
    /** Promise to track active starting to prevent race conditions. */
    private startingPromise: Promise<void> | null = null;
    /** Configuration of the currently running recognizer. */
    private runningConfig: ServiceConfig | null = null;
    /** Language code for transcription. */
    private language: string = 'auto';

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
     * Sets the language for transcription.
     *
     * @param language The language code (e.g., 'en', 'zh', 'auto').
     */
    setLanguage(language: string): void {
        this.language = language;
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
            console.log('[TranscriptionService] Configuration changed, restarting backend...');
            await this.stop();
        }

        if (!this.modelPath) {
            console.warn('[TranscriptionService] Model path not configured, cannot prepare backend');
            return;
        }

        console.log('[TranscriptionService] Pre-starting backend...');
        return this._startBackend();
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
            console.log('[TranscriptionService] Configuration changed, restarting backend for start...');
            await this.stop();
        }

        if (!this.modelPath) {
            onError('Model path not configured');
            return;
        }

        await this._startBackend();
    }

    private async _startBackend(): Promise<void> {
        if (this.startingPromise) {
            return this.startingPromise;
        }

        this.startingPromise = (async () => {
            console.log('Starting Rust backend recognizer with model:', this.modelPath);

            const configToUse: ServiceConfig = {
                modelPath: this.modelPath,
                itnModelPaths: [...this.itnModelPaths],
                punctuationModelPath: this.punctuationModelPath,
                vadModelPath: this.vadModelPath,
                vadBufferSize: this.vadBufferSize,
                enableITN: this.enableITN,
                language: this.language
            };

            try {
                if (!this.unlistenOutput) {
                    this.unlistenOutput = await listen<TranscriptSegment>('recognizer-output', (event) => {
                        const segment = event.payload;
                        if (this.onSegment) {
                            this.onSegment(segment);
                        }
                    });
                }

                await invoke('start_recognizer', {
                    modelPath: this.modelPath,
                    numThreads: 4,
                    enableItn: this.enableITN,
                    language: this.language,
                    itnModel: this.itnModelPaths.length > 0 ? this.itnModelPaths.join(',') : null,
                    punctuationModel: this.punctuationModelPath || null,
                    vadModel: this.vadModelPath || null,
                    vadBuffer: this.vadBufferSize || 5.0
                });

                this.isRunning = true;
                this.runningConfig = configToUse;
                console.log('[TranscriptionService] Rust Recognizer started');

            } catch (error) {
                console.error('Failed to start recognizer:', error);
                if (this.onError) this.onError(`Failed to start: ${error}`);
                this.isRunning = false;
                this.runningConfig = null;
                throw error;
            }
        })();

        try {
            await this.startingPromise;
        } finally {
            this.startingPromise = null;
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
        if (this.language !== this.runningConfig.language) return false;

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
        if (!this.isRunning) return;

        try {
            await invoke('stop_recognizer');
        } catch (error) {
            console.error('Failed to stop recognizer:', error);
        } finally {
            console.log('[TranscriptionService] Recognizer stopped');
            this.isRunning = false;
            this.runningConfig = null;
            if (this.unlistenOutput) {
                this.unlistenOutput();
                this.unlistenOutput = null;
            }
        }
    }


    /**
     * Stops the transcription gracefully by sending an end-of-stream signal.
     *
     * This allows the model to finish the last segment and add punctuation.
     */
    async softStop(): Promise<void> {
        await this.stop();
    }

    /**
     * Completely terminates the process.
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
        if (!this.isRunning) return;

        try {
            const floatSamples = new Float32Array(samples.length);
            for (let i = 0; i < samples.length; i++) {
                floatSamples[i] = samples[i] / 32768.0;
            }
            await invoke('feed_audio_chunk', { samples: Array.from(floatSamples) });
        } catch (error) {
            console.error('Failed to feed audio to backend:', error);
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
    private async _transcribeFileInternal(filePath: string, _provider?: string, onProgress?: (progress: number) => void, onSegment?: TranscriptionCallback, language?: string, _saveToPath?: string): Promise<TranscriptSegment[]> {
        if (!this.modelPath) {
            throw new Error('Model path not configured');
        }

        console.log(`[TranscriptionService] Starting batch transcription for: ${filePath} via Rust backend`);

        try {
            // Need to make sure the recognizer is initialized for batch (Offline)
            await invoke('start_recognizer', {
                modelPath: this.modelPath,
                numThreads: 4,
                enableItn: this.enableITN,
                language: language || this.language || 'auto',
                itnModel: this.itnModelPaths.length > 0 ? this.itnModelPaths.join(',') : null,
                punctuationModel: this.punctuationModelPath || null,
                vadModel: this.vadModelPath || null,
                vadBuffer: this.vadBufferSize || 5.0
            });

            let progressUnlisten: UnlistenFn | undefined;
            if (onProgress) {
                progressUnlisten = await listen<[string, number]>('batch-progress', (event) => {
                    const [eventFilePath, progress] = event.payload;
                    if (eventFilePath === filePath) {
                        onProgress(progress);
                    }
                });
            }

            const segments = await invoke<TranscriptSegment[]>('process_batch_file', {
                filePath: filePath,
                saveToPath: _saveToPath || null
            });

            if (progressUnlisten) {
                progressUnlisten();
            }

            if (onProgress) {
                onProgress(100);
            }

            if (onSegment) {
                segments.forEach(seg => {
                    // Filter out single period segments when final if needed (similar to _createSegment)
                    if (seg.isFinal) {
                        const trimmedText = seg.text.trim();
                        if (trimmedText === '.' || trimmedText === '。') {
                            return;
                        }
                    }
                    onSegment(seg);
                });
            }

            return segments;
        } catch (error) {
            console.error('[TranscriptionService] Batch transcription failed:', error);
            throw new Error(`Process error: ${error}`);
        }
    }

    /**
     * Emits a segment as if it came from the backend.
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
     * Returns null as it is currently unimplmented in the Rust backend.
     *
     * @param segment The segment to align (uses start/end times).
     * @param sourceFilePath Optional override for the audio file path.
     * @return The alignment result, or null if alignment is unavailable or fails.
     */
    async alignSegment(segment: TranscriptSegment, _sourceFilePath?: string): Promise<AlignmentResult | null> {
        console.warn(`[TranscriptionService] CTC Alignment is not yet implemented in the Rust backend. Returning null for segment ${segment.id}`);
        return null;
    }
}

export const transcriptionService = new TranscriptionService();
