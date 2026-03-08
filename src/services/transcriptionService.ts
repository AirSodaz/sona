import { logger } from "../utils/logger";
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { TranscriptSegment } from '../types/transcript';
import { useTranscriptStore } from '../stores/transcriptStore';
import { PRESET_MODELS, modelService } from './modelService';
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
    /** Unique ID for the recognizer instance. */
    private instanceId: string;

    /**
     * Initializes a new instance of the TranscriptionService.
     * @param instanceId A unique string identifier for this instance.
     */
    constructor(instanceId: string = 'default') {
        this.instanceId = instanceId;
    }

    /**
     * Sets the path to the main ASR model.
     *
     * @param path The absolute path to the model file or directory.
     */
    setModelPath(path: string): void {
        logger.info('[TranscriptionService] Setting model path:', path);
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
     * Enables or disables Inverse Text Normalization (ITN).
     *
     * @param enabled True to enable ITN, or false to disable it.
     */
    setEnableITN(enabled: boolean): void {
        this.enableITN = enabled;
    }

    /**
     * Initializes the recognizer models in the background.
     */
    async prepare(): Promise<void> {
        if (this._isConfigMatch()) {
            return;
        }

        if (!this.modelPath) {
            logger.warn('[TranscriptionService] Model path not configured, cannot prepare backend');
            return;
        }

        logger.info('[TranscriptionService] Initializing recognizer models...');
        return this._initBackend();
    }

    async start(onSegment: TranscriptionCallback, onError: ErrorCallback): Promise<void> {
        this.onSegment = onSegment;
        this.onError = onError;

        if (!this.modelPath) {
            onError('Model path not configured');
            return;
        }

        // If config changed, re-initialize first
        if (!this._isConfigMatch()) {
            logger.info('[TranscriptionService] Configuration changed, re-initializing backend for start...');
            await this._initBackend();
        }

        await this._startStream();
    }

    private async _initBackend(): Promise<void> {
        if (this.startingPromise) {
            return this.startingPromise;
        }

        this.startingPromise = (async () => {
            logger.info(`[TranscriptionService:${this.instanceId}] Initializing Rust backend recognizer with model: ${this.modelPath}`);

            // Fetch app config for VAD/Punctuation paths
            const appConfig = useTranscriptStore.getState().config;

            // Determine rules based on the streaming model ID
            let punctuationPathToUse = '';
            let vadPathToUse = '';
            let vadBufferToUse = 5.0;

            const streamingModel = PRESET_MODELS.find(m => (m.type === 'sensevoice' || m.type === 'paraformer') && m.modes?.includes('streaming') && this.modelPath.includes(m.filename || m.id));
            if (streamingModel) {
                const rules = modelService.getModelRules(streamingModel.id);
                if (rules.requiresPunctuation && appConfig.punctuationModelPath) {
                    punctuationPathToUse = appConfig.punctuationModelPath;
                }
                if (rules.requiresVad && appConfig.vadModelPath) {
                    vadPathToUse = appConfig.vadModelPath;
                    vadBufferToUse = appConfig.vadBufferSize || 5.0;
                }
            }

            const configToUse: ServiceConfig = {
                modelPath: this.modelPath,
                itnModelPaths: [...this.itnModelPaths],
                punctuationModelPath: punctuationPathToUse,
                vadModelPath: vadPathToUse,
                vadBufferSize: vadBufferToUse,
                enableITN: this.enableITN,
                language: this.language
            };

            try {
                await invoke('init_recognizer', {
                    instanceId: this.instanceId,
                    modelPath: this.modelPath,
                    numThreads: 4,
                    enableItn: this.enableITN,
                    language: this.language,
                    itnModel: this.itnModelPaths.length > 0 ? this.itnModelPaths.join(',') : null,
                    punctuationModel: punctuationPathToUse || null,
                    vadModel: vadPathToUse || null,
                    vadBuffer: vadBufferToUse
                });

                this.runningConfig = configToUse;
                logger.info(`[TranscriptionService:${this.instanceId}] Rust Recognizer initialized`);

            } catch (error) {
                logger.error(`[TranscriptionService:${this.instanceId}] Failed to initialize recognizer:`, error);
                if (this.onError) this.onError(`Failed to initialize: ${error}`);
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

    private async _startStream(): Promise<void> {
        try {
            if (!this.unlistenOutput) {
                const eventName = `recognizer-output-${this.instanceId}`;
                this.unlistenOutput = await listen<TranscriptSegment>(eventName, (event) => {
                    const segment = event.payload;
                    if (this.onSegment) {
                        this.onSegment(segment);
                    }
                });
            }

            await invoke('start_recognizer', { instanceId: this.instanceId });

            this.isRunning = true;
            logger.info('[TranscriptionService] Rust Recognizer stream started');

        } catch (error) {
            logger.error('Failed to start recognizer stream:', error);
            if (this.onError) this.onError(`Failed to start stream: ${error}`);
            this.isRunning = false;
            throw error;
        }
    }

    /**
     * Checks if the current configuration matches the running configuration.
     */
    private _isConfigMatch(): boolean {
        if (!this.runningConfig) return false;

        if (this.modelPath !== this.runningConfig.modelPath) return false;
        if (this.enableITN !== this.runningConfig.enableITN) return false;
        if (this.language !== this.runningConfig.language) return false;

        // Compare dynamically resolved VAD and Punctuation against what is running
        const appConfig = useTranscriptStore.getState().config;
        let punctuationPathToUse = '';
        let vadPathToUse = '';
        let vadBufferToUse = 5.0;

        const streamingModel = PRESET_MODELS.find(m => (m.type === 'sensevoice' || m.type === 'paraformer') && m.modes?.includes('streaming') && this.modelPath.includes(m.filename || m.id));
        if (streamingModel) {
            const rules = modelService.getModelRules(streamingModel.id);
            if (rules.requiresPunctuation && appConfig.punctuationModelPath) {
                punctuationPathToUse = appConfig.punctuationModelPath;
            }
            if (rules.requiresVad && appConfig.vadModelPath) {
                vadPathToUse = appConfig.vadModelPath;
                vadBufferToUse = appConfig.vadBufferSize || 5.0;
            }
        }

        if (punctuationPathToUse !== this.runningConfig.punctuationModelPath) return false;
        if (vadPathToUse !== this.runningConfig.vadModelPath) return false;
        if (vadBufferToUse !== this.runningConfig.vadBufferSize) return false;


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
            await invoke('stop_recognizer', { instanceId: this.instanceId });
        } catch (error) {
            logger.error(`[TranscriptionService:${this.instanceId}] Failed to stop recognizer:`, error);
        } finally {
            logger.info(`[TranscriptionService:${this.instanceId}] Recognizer stopped`);
            this.isRunning = false;
            // We intentionally do NOT set this.runningConfig = null here.
            // Keeping the runningConfig allows subsequent starts with the same config
            // to bypass the expensive _initBackend() call, as the Rust backend keeps the models loaded.
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
        if (this.isRunning) {
            try {
                await invoke('flush_recognizer', { instanceId: this.instanceId });
            } catch (error) {
                logger.error(`[TranscriptionService:${this.instanceId}] Failed to flush recognizer:`, error);
            }
        }
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
            // Send the raw bytes of the Int16Array to match the new backend signature (Vec<u8>)
            const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
            await invoke('feed_audio_chunk', { instanceId: this.instanceId, samples: bytes });
        } catch (error) {
            logger.error(`[TranscriptionService:${this.instanceId}] Failed to feed audio to backend:`, error);
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
            // Check if the error message contains 'COREML_FAILURE' inside the wrapped error
            if (error.message && error.message.includes('COREML_FAILURE')) {
                logger.warn('[TranscriptionService] CoreML failure detected. Retrying with CPU...');
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

        logger.info(`[TranscriptionService] Starting batch transcription for: ${filePath} via Rust backend`);

        try {
            let progressUnlisten: UnlistenFn | undefined;
            if (onProgress) {
                progressUnlisten = await listen<[string, number]>('batch-progress', (event) => {
                    const [eventFilePath, progress] = event.payload;
                    if (eventFilePath === filePath) {
                        onProgress(progress);
                    }
                });
            }

            const appConfig = useTranscriptStore.getState().config;
            let punctuationPathToUse = '';
            let vadPathToUse = '';
            let vadBufferToUse = 5.0;

            const offlineModel = PRESET_MODELS.find(m => m.type === 'sensevoice' && m.modes?.includes('offline') && this.modelPath.includes(m.filename || m.id));
            if (offlineModel) {
                const rules = modelService.getModelRules(offlineModel.id);
                if (rules.requiresPunctuation && appConfig.punctuationModelPath) {
                    punctuationPathToUse = appConfig.punctuationModelPath;
                }
                if (rules.requiresVad && appConfig.vadModelPath) {
                    vadPathToUse = appConfig.vadModelPath;
                    vadBufferToUse = appConfig.vadBufferSize || 5.0;
                }
            }

            const segments = await invoke<TranscriptSegment[]>('process_batch_file', {
                filePath: filePath,
                saveToPath: _saveToPath || null,
                modelPath: this.modelPath,
                numThreads: 4,
                enableItn: this.enableITN,
                language: language || this.language || 'auto',
                itnModel: this.itnModelPaths.length > 0 ? this.itnModelPaths.join(',') : null,
                punctuationModel: punctuationPathToUse || null,
                vadModel: vadPathToUse || null,
                vadBuffer: vadBufferToUse
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
            logger.error('[TranscriptionService] Batch transcription failed:', error);
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
        logger.warn(`[TranscriptionService] CTC Alignment is not yet implemented in the Rust backend. Returning null for segment ${segment.id}`);
        return null;
    }
}

export const transcriptionService = new TranscriptionService('record');
export const captionTranscriptionService = new TranscriptionService('caption');
