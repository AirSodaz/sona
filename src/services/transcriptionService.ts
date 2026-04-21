import { logger } from "../utils/logger";
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { TranscriptSegment } from '../types/transcript';
import { useConfigStore } from '../stores/configStore';
import { PRESET_MODELS, modelService, ModelFileConfig } from './modelService';
import { applyTextReplacements } from '../utils/textProcessing';

/** Callback for receiving a new transcript segment. */
export type TranscriptionCallback = (segment: TranscriptSegment) => void;
/** Callback for receiving an error message. */
export type ErrorCallback = (error: string) => void;

interface StartOptions {
    callbackOwner?: string;
    callbackSessionId?: string | null;
}

interface ServiceConfig {
    modelPath: string;
    punctuationModelPath: string;
    vadModelPath: string;
    vadBufferSize: number;
    enableITN: boolean;
    language: string;
    modelType: string;
    fileConfig?: ModelFileConfig;
    hotwords?: string;
}

/**
 * Service to manage the transcription process via the Rust backend.
 * Uses a Global Bus pattern for event reliability.
 */
export class TranscriptionService {
    private static globalListeners: Map<string, UnlistenFn> = new Map();
    private static callbackRegistrationCounter = 0;
    private static instanceCallbacks: Map<string, {
        onSegment: TranscriptionCallback,
        onError: ErrorCallback,
        owner: string,
        sessionId: string | null,
        registrationId: number,
    }> = new Map();

    private static isDiagnosticsInstance(instanceId: string): boolean {
        return instanceId === 'record' || instanceId === 'voice-typing';
    }

    private static formatSession(sessionId: string | null | undefined): string {
        return sessionId ?? 'none';
    }

    /** Ensures the global listener is active for a specific instance. */
    private static async ensureGlobalBusFor(instanceId: string) {
        if (this.globalListeners.has(instanceId)) return;

        const eventName = `recognizer-output-${instanceId}`;
        const unlisten = await listen<TranscriptSegment>(eventName, (event) => {
            const segment = event.payload;
            const instance = this.instanceCallbacks.get(instanceId);
            if (!instance) {
                if (this.isDiagnosticsInstance(instanceId)) {
                    logger.info(
                        `[TranscriptionService:${instanceId}] Received recognizer event without an active callback. segment=${segment.id} final=${segment.isFinal}`
                    );
                }
                return;
            }

            if (this.isDiagnosticsInstance(instanceId)) {
                logger.info(
                    `[TranscriptionService:${instanceId}] Received recognizer event. registration=${instance.registrationId} owner=${instance.owner} session=${this.formatSession(instance.sessionId)} segment=${segment.id} final=${segment.isFinal} textLength=${segment.text.length}`
                );
            }

            try {
                // Apply text replacements from global config
                const appConfig = useConfigStore.getState().config;
                const originalText = segment.text;
                const processedText = applyTextReplacements(originalText, appConfig.textReplacementSets);

                if (originalText !== processedText) {
                    logger.debug(`[TranscriptionService:BUS] Replaced text in ${instanceId}: "${originalText}" -> "${processedText}"`);
                }
                const processedSegment = {
                    ...segment,
                    text: processedText
                };
                instance.onSegment(processedSegment);
            } catch (e) {
                logger.error(`[TranscriptionService:BUS] Error in ${instanceId} callback:`, e);
            }
        });

        this.globalListeners.set(instanceId, unlisten);
    }

    private isRunning: boolean = false;
    private modelPath: string = '';
    private enableITN: boolean = true;
    private onError: ErrorCallback | null = null;
    private startingPromise: Promise<void> | null = null;
    private runningConfig: ServiceConfig | null = null;
    private language: string = 'auto';
    private instanceId: string;

    constructor(instanceId: string = 'default') {
        this.instanceId = instanceId;
    }

    setModelPath(path: string): void {
        this.modelPath = path;
    }

    setLanguage(language: string): void {
        this.language = language;
    }

    setEnableITN(enabled: boolean): void {
        this.enableITN = enabled;
    }

    async prepare(): Promise<void> {
        if (this._isConfigMatch()) return;
        if (!this.modelPath) return;
        return this._initBackend();
    }

    async start(onSegment: TranscriptionCallback, onError: ErrorCallback, options?: StartOptions): Promise<void> {
        this.onError = onError;

        if (!this.modelPath) {
            onError('Model path not configured');
            return;
        }

        const existingRegistration = TranscriptionService.instanceCallbacks.get(this.instanceId);
        if (
            TranscriptionService.isDiagnosticsInstance(this.instanceId) &&
            existingRegistration
        ) {
            logger.info(
                `[TranscriptionService:${this.instanceId}] Replacing callback registration. previous_registration=${existingRegistration.registrationId} previous_owner=${existingRegistration.owner} previous_session=${TranscriptionService.formatSession(existingRegistration.sessionId)}`
            );
        }

        const owner = options?.callbackOwner ?? this.instanceId;
        const sessionId = options?.callbackSessionId ?? null;
        const registrationId = ++TranscriptionService.callbackRegistrationCounter;
        const wrappedOnSegment: TranscriptionCallback = (segment) => {
            const currentRegistration = TranscriptionService.instanceCallbacks.get(this.instanceId);
            if (!currentRegistration || currentRegistration.registrationId !== registrationId) {
                if (TranscriptionService.isDiagnosticsInstance(this.instanceId)) {
                    logger.info(
                        `[TranscriptionService:${this.instanceId}] Ignored stale callback invocation. registration=${registrationId} owner=${owner} session=${TranscriptionService.formatSession(sessionId)} current_registration=${currentRegistration?.registrationId ?? 'none'}`
                    );
                }
                return;
            }
            onSegment(segment);
        };

        TranscriptionService.instanceCallbacks.set(this.instanceId, {
            onSegment: wrappedOnSegment,
            onError,
            owner,
            sessionId,
            registrationId
        });

        if (TranscriptionService.isDiagnosticsInstance(this.instanceId)) {
            logger.info(
                `[TranscriptionService:${this.instanceId}] Registered callback. registration=${registrationId} owner=${owner} session=${TranscriptionService.formatSession(sessionId)}`
            );
        }

        await TranscriptionService.ensureGlobalBusFor(this.instanceId);

        if (!this._isConfigMatch()) {
            await this._initBackend();
        }

        await this._startStream();
    }

    private async _initBackend(): Promise<void> {
        if (this.startingPromise) return this.startingPromise;

        this.startingPromise = (async () => {
            try {
                const appConfig = useConfigStore.getState().config;
                let punctuationPathToUse = '';
                let vadPathToUse = '';
                let vadBufferToUse = 5.0;

                const streamingModel = PRESET_MODELS.find(m => m.modes?.includes('streaming') && this.modelPath.includes(m.filename || m.id));

                if (streamingModel) {
                    const rules = modelService.getModelRules(streamingModel.id);
                    if (rules.requiresPunctuation && appConfig.punctuationModelPath) {
                        punctuationPathToUse = appConfig.punctuationModelPath;
                    }
                    if (rules.requiresVad) {
                        if (appConfig.vadModelPath) {
                            vadPathToUse = appConfig.vadModelPath;
                            vadBufferToUse = appConfig.vadBufferSize || 5.0;
                        } else {
                            const errorMsg = 'VAD model not configured. Please download the Silero VAD model in Settings → Model Center.';
                            if (this.onError) this.onError(errorMsg);
                            throw new Error(errorMsg);
                        }
                    }
                }

                const enabledHotwords = appConfig.hotwordSets
                    ?.filter(set => set.enabled)
                    .flatMap(set => set.rules.map(r => r.text))
                    .filter(text => text.trim() !== '') || [];
                const hotwordsStr = enabledHotwords.length > 0 ? enabledHotwords.join(',') : undefined;

                const configToUse: ServiceConfig = {
                    modelPath: this.modelPath,
                    punctuationModelPath: punctuationPathToUse,
                    vadModelPath: vadPathToUse,
                    vadBufferSize: vadBufferToUse,
                    enableITN: this.enableITN,
                    language: this.language,
                    modelType: streamingModel?.type || 'sensevoice',
                    fileConfig: streamingModel?.fileConfig,
                    hotwords: hotwordsStr
                };

                await invoke('init_recognizer', {
                    instanceId: this.instanceId,
                    modelPath: configToUse.modelPath,
                    numThreads: 4,
                    enableItn: configToUse.enableITN,
                    language: configToUse.language,
                    punctuationModel: configToUse.punctuationModelPath || null,
                    vadModel: configToUse.vadModelPath || null,
                    vadBuffer: configToUse.vadBufferSize,
                    modelType: configToUse.modelType,
                    fileConfig: configToUse.fileConfig,
                    hotwords: configToUse.hotwords || null,
                });

                this.runningConfig = configToUse;
            } catch (error) {
                logger.error(`[TranscriptionService:${this.instanceId}] Failed to initialize:`, error);
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
            await invoke('start_recognizer', { instanceId: this.instanceId });
            this.isRunning = true;
        } catch (error) {
            logger.error(`[TranscriptionService:${this.instanceId}] Failed to start stream:`, error);
            if (this.onError) this.onError(`Failed to start stream: ${error}`);
            this.isRunning = false;
            throw error;
        }
    }

    private _isConfigMatch(): boolean {
        if (!this.runningConfig) return false;

        const appConfig = useConfigStore.getState().config;
        let vadPathToUse = '';
        let punctuationPathToUse = '';
        const streamingModel = PRESET_MODELS.find(m => m.modes?.includes('streaming') && this.modelPath.includes(m.filename || m.id));
        
        if (streamingModel) {
            const rules = modelService.getModelRules(streamingModel.id);
            if (rules.requiresVad && appConfig.vadModelPath) {
                vadPathToUse = appConfig.vadModelPath;
            }
            if (rules.requiresPunctuation && appConfig.punctuationModelPath) {
                punctuationPathToUse = appConfig.punctuationModelPath;
            }
        }

        const enabledHotwords = appConfig.hotwordSets
            ?.filter(set => set.enabled)
            .flatMap(set => set.rules.map(r => r.text))
            .filter(text => text.trim() !== '') || [];
        const hotwordsStr = enabledHotwords.length > 0 ? enabledHotwords.join(',') : undefined;

        const modelType = streamingModel?.type || 'sensevoice';

        const mismatches: string[] = [];
        if (this.modelPath !== this.runningConfig.modelPath) mismatches.push(`modelPath: ${this.modelPath} vs ${this.runningConfig.modelPath}`);
        if (this.enableITN !== this.runningConfig.enableITN) mismatches.push(`enableITN: ${this.enableITN} vs ${this.runningConfig.enableITN}`);
        if (this.language !== this.runningConfig.language) mismatches.push(`language: ${this.language} vs ${this.runningConfig.language}`);
        if (vadPathToUse !== this.runningConfig.vadModelPath) mismatches.push(`vadModelPath: ${vadPathToUse} vs ${this.runningConfig.vadModelPath}`);
        if (punctuationPathToUse !== this.runningConfig.punctuationModelPath) mismatches.push(`punctuationModelPath: ${punctuationPathToUse} vs ${this.runningConfig.punctuationModelPath}`);
        if (hotwordsStr !== this.runningConfig.hotwords) mismatches.push(`hotwords: ${hotwordsStr} vs ${this.runningConfig.hotwords}`);
        if (modelType !== this.runningConfig.modelType) mismatches.push(`modelType: ${modelType} vs ${this.runningConfig.modelType}`);

        if (mismatches.length > 0) {
            logger.info(`[TranscriptionService:${this.instanceId}] Config mismatch detected. Model will be re-initialized.`, mismatches);
            return false;
        }

        return true;
    }

    async stop(): Promise<void> {
        if (!this.isRunning) return;
        try {
            await invoke('stop_recognizer', { instanceId: this.instanceId });
        } finally {
            this.isRunning = false;
        }
    }

    async softStop(): Promise<void> {
        if (this.isRunning) {
            try {
                await invoke('flush_recognizer', { instanceId: this.instanceId });
            } catch (error) {
                logger.error('Flush failed:', error);
            }
        }
        await this.stop();
    }

    async terminate(): Promise<void> {
        await this.stop();
    }

    async sendAudioInt16(samples: Int16Array): Promise<void> {
        if (!this.isRunning) return;
        try {
            const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
            await invoke('feed_audio_chunk', { instanceId: this.instanceId, samples: bytes });
        } catch (error) {
            logger.error('Feed audio failed:', error);
        }
    }

    async transcribeFile(filePath: string, onProgress?: (progress: number) => void, onSegment?: TranscriptionCallback, language?: string, saveToPath?: string): Promise<TranscriptSegment[]> {
        try {
            return await this._transcribeFileInternal(filePath, undefined, onProgress, onSegment, language, saveToPath);
        } catch (error: any) {
            if (error.message && error.message.includes('COREML_FAILURE')) {
                return await this._transcribeFileInternal(filePath, 'cpu', onProgress, onSegment, language, saveToPath);
            }
            throw error;
        }
    }

    private async _transcribeFileInternal(filePath: string, _provider?: string, onProgress?: (progress: number) => void, onSegment?: TranscriptionCallback, language?: string, _saveToPath?: string): Promise<TranscriptSegment[]> {
        if (!this.modelPath) throw new Error('Model path not configured');

        const appConfig = useConfigStore.getState().config;
        let punctuationPathToUse = '';
        let vadPathToUse = '';
        let vadBufferToUse = 5.0;

        const offlineModel = PRESET_MODELS.find(m => m.modes?.includes('offline') && this.modelPath.includes(m.filename || m.id));
        if (offlineModel) {
            const rules = modelService.getModelRules(offlineModel.id);
            if (rules.requiresPunctuation && appConfig.punctuationModelPath) punctuationPathToUse = appConfig.punctuationModelPath;
            if (rules.requiresVad && appConfig.vadModelPath) {
                vadPathToUse = appConfig.vadModelPath;
                vadBufferToUse = appConfig.vadBufferSize || 5.0;
            }
        }

        const enabledHotwords = appConfig.hotwordSets
            ?.filter(set => set.enabled)
            .flatMap(set => set.rules.map(r => r.text))
            .filter(text => text.trim() !== '') || [];
        const hotwordsStr = enabledHotwords.length > 0 ? enabledHotwords.join(',') : null;

        const segments = await invoke<TranscriptSegment[]>('process_batch_file', {
            filePath, saveToPath: _saveToPath || null, modelPath: this.modelPath, numThreads: 4, enableItn: this.enableITN,
            language: language || this.language || 'auto', punctuationModel: punctuationPathToUse || null,
            vadModel: vadPathToUse || null, vadBuffer: vadBufferToUse, modelType: offlineModel?.type || 'sensevoice',
            fileConfig: offlineModel?.fileConfig,
            hotwords: hotwordsStr
        });

        // Filter segments: some models (like Whisper) occasionally produce single "." segments
        const filteredSegments = segments.filter(seg => !(seg.text === '.' && seg.isFinal));
        
        // Apply text replacements
        const processedSegments = filteredSegments.map(seg => ({
            ...seg,
            text: applyTextReplacements(seg.text, appConfig.textReplacementSets)
        }));

        if (onProgress) onProgress(100);
        if (onSegment) processedSegments.forEach(seg => onSegment(seg));
        return processedSegments;
    }
}

export const transcriptionService = new TranscriptionService('record');
export const captionTranscriptionService = new TranscriptionService('caption');
