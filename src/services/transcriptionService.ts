import { logger } from "../utils/logger";
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import type { AppConfig } from '../types/config';
import { useTranscriptStore } from '../stores/transcriptStore';
import { modelService, ModelFileConfig } from './modelService';
import { applyTextReplacements } from '../utils/textProcessing';
import { speakerService } from './speakerService';
import { extractErrorMessage } from '../utils/errorUtils';
import { findSelectedModelByMode } from '../utils/modelSelection';
import { normalizeTranscriptSegments, normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import { buildRecognizerOutputEvent } from './tauri/events';
import {
    feedAudioChunk,
    flushRecognizer,
    initRecognizer,
    processBatchFile,
    startRecognizer,
    stopRecognizer,
} from './tauri/recognizer';

const LOG_PREVIEW_MAX_CHARS = 24;

function previewTextForLog(text: string) {
    const flattened = text.replace(/\r?\n/g, ' ');
    const preview = flattened.slice(0, LOG_PREVIEW_MAX_CHARS);
    return flattened.length > LOG_PREVIEW_MAX_CHARS ? `${preview}...` : preview;
}

function shouldLogVoiceTypingDiagnostics(instanceId: string) {
    return instanceId === 'voice-typing';
}

/** Callback for receiving a normalized streaming transcript update. */
export type TranscriptionUpdateCallback = (update: TranscriptUpdate) => void;
/** Callback for receiving a batch transcript segment. */
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
    enableTimeline: boolean;
}

/**
 * Service to manage the transcription process via the Rust backend.
 * Uses a Global Bus pattern for event reliability.
 */
export class TranscriptionService {
    private static globalListeners: Map<string, UnlistenFn> = new Map();
    private static callbackRegistrationCounter = 0;
    private static instanceCallbacks: Map<string, {
        onUpdate: TranscriptionUpdateCallback,
        onError: ErrorCallback,
        owner: string,
        sessionId: string | null,
        registrationId: number,
    }> = new Map();

    private static isDiagnosticsInstance(instanceId: string): boolean {
        return (
            instanceId === 'record' ||
            instanceId === 'voice-typing' ||
            instanceId === 'caption'
        );
    }

    private static formatSession(sessionId: string | null | undefined): string {
        return sessionId ?? 'none';
    }

    /** Ensures the global listener is active for a specific instance. */
    private static async ensureGlobalBusFor(instanceId: string) {
        if (this.globalListeners.has(instanceId)) return;

        const eventName = buildRecognizerOutputEvent(instanceId);
        const unlisten = await listen<TranscriptUpdate>(eventName, (event) => {
            const update = normalizeTranscriptUpdate(event.payload);
            const instance = this.instanceCallbacks.get(instanceId);
            if (!instance) {
                if (this.isDiagnosticsInstance(instanceId)) {
                    logger.info(
                        `[TranscriptionService:${instanceId}] Received recognizer event without an active callback. removes=${update.removeIds.length} upserts=${update.upsertSegments.length}`
                    );
                }
                return;
            }

            if (this.isDiagnosticsInstance(instanceId)) {
                logger.info(
                    `[TranscriptionService:${instanceId}] Received recognizer event. registration=${instance.registrationId} owner=${instance.owner} session=${this.formatSession(instance.sessionId)} removes=${update.removeIds.length} upserts=${update.upsertSegments.length}`
                );
            }

            try {
                // Apply text replacements from global config
                const appConfig = useTranscriptStore.getState().config;
                const processedUpdate = normalizeTranscriptUpdate({
                    removeIds: update.removeIds,
                    upsertSegments: update.upsertSegments.map((segment) => {
                        const originalText = segment.text;
                        const processedText = applyTextReplacements(originalText, appConfig.textReplacementSets);
                        const replacementChanged = originalText !== processedText;
                        const replacedToEmpty = originalText.trim().length > 0 && processedText.trim().length === 0;

                        if (shouldLogVoiceTypingDiagnostics(instanceId)) {
                            logger.info(
                                '[TranscriptionService:voice-typing] Prepared recognizer segment for callback',
                                {
                                    instanceId,
                                    registrationId: instance.registrationId,
                                    segmentId: segment.id,
                                    isFinal: segment.isFinal,
                                    rawTextLength: originalText.length,
                                    processedTextLength: processedText.length,
                                    preview: previewTextForLog(processedText || originalText),
                                    replacementChanged,
                                    replacedToEmpty,
                                    callbackInvoked: false,
                                }
                            );
                        }

                        if (replacementChanged) {
                            logger.debug(`[TranscriptionService:BUS] Replaced text in ${instanceId}: "${originalText}" -> "${processedText}"`);
                        }

                        return {
                            ...segment,
                            text: processedText,
                        };
                    }),
                });

                instance.onUpdate(processedUpdate);
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

    async start(onUpdate: TranscriptionUpdateCallback, onError: ErrorCallback, options?: StartOptions): Promise<void> {
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
        const wrappedOnUpdate: TranscriptionUpdateCallback = (update) => {
            const currentRegistration = TranscriptionService.instanceCallbacks.get(this.instanceId);
            if (!currentRegistration || currentRegistration.registrationId !== registrationId) {
                if (shouldLogVoiceTypingDiagnostics(this.instanceId)) {
                    logger.info(
                        '[TranscriptionService:voice-typing] Skipped callback invocation',
                        {
                            instanceId: this.instanceId,
                            registrationId,
                            currentRegistrationId: currentRegistration?.registrationId ?? null,
                            segmentIds: update.upsertSegments.map((segment) => segment.id),
                            removeIds: update.removeIds,
                            rawTextLength: null,
                            processedTextLength: update.upsertSegments.reduce((sum, segment) => sum + segment.text.length, 0),
                            preview: previewTextForLog(update.upsertSegments.map((segment) => segment.text).join(' ')),
                            callbackInvoked: false,
                            dropReason: 'stale_registration',
                        }
                    );
                }
                if (TranscriptionService.isDiagnosticsInstance(this.instanceId)) {
                    logger.info(
                        `[TranscriptionService:${this.instanceId}] Ignored stale callback invocation. registration=${registrationId} owner=${owner} session=${TranscriptionService.formatSession(sessionId)} current_registration=${currentRegistration?.registrationId ?? 'none'}`
                    );
                }
                return;
            }

            if (shouldLogVoiceTypingDiagnostics(this.instanceId)) {
                logger.info(
                    '[TranscriptionService:voice-typing] Invoking callback',
                    {
                        instanceId: this.instanceId,
                        registrationId,
                        segmentIds: update.upsertSegments.map((segment) => segment.id),
                        removeIds: update.removeIds,
                        rawTextLength: null,
                        processedTextLength: update.upsertSegments.reduce((sum, segment) => sum + segment.text.length, 0),
                        preview: previewTextForLog(update.upsertSegments.map((segment) => segment.text).join(' ')),
                        callbackInvoked: true,
                    }
                );
            }
            onUpdate(update);
        };

        TranscriptionService.instanceCallbacks.set(this.instanceId, {
            onUpdate: wrappedOnUpdate,
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
                const appConfig = useTranscriptStore.getState().config;
                let punctuationPathToUse = '';
                let vadPathToUse = '';
                let vadBufferToUse = 5.0;

                const streamingModel = findSelectedModelByMode(this.modelPath, 'streaming');

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
                    hotwords: hotwordsStr,
                    enableTimeline: this.instanceId === 'record' ? (appConfig.enableTimeline ?? false) : false,
                };

                await initRecognizer({
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
                    normalizationOptions: {
                        enableTimeline: configToUse.enableTimeline,
                    },
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
        if (this.isRunning) {
            return;
        }
        try {
            await startRecognizer(this.instanceId);
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

        const appConfig = useTranscriptStore.getState().config;
        let vadPathToUse = '';
        let punctuationPathToUse = '';
        const streamingModel = findSelectedModelByMode(this.modelPath, 'streaming');
        
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
        const enableTimeline = this.instanceId === 'record' ? (appConfig.enableTimeline ?? false) : false;

        const mismatches: string[] = [];
        if (this.modelPath !== this.runningConfig.modelPath) mismatches.push(`modelPath: ${this.modelPath} vs ${this.runningConfig.modelPath}`);
        if (this.enableITN !== this.runningConfig.enableITN) mismatches.push(`enableITN: ${this.enableITN} vs ${this.runningConfig.enableITN}`);
        if (this.language !== this.runningConfig.language) mismatches.push(`language: ${this.language} vs ${this.runningConfig.language}`);
        if (vadPathToUse !== this.runningConfig.vadModelPath) mismatches.push(`vadModelPath: ${vadPathToUse} vs ${this.runningConfig.vadModelPath}`);
        if (punctuationPathToUse !== this.runningConfig.punctuationModelPath) mismatches.push(`punctuationModelPath: ${punctuationPathToUse} vs ${this.runningConfig.punctuationModelPath}`);
        if (hotwordsStr !== this.runningConfig.hotwords) mismatches.push(`hotwords: ${hotwordsStr} vs ${this.runningConfig.hotwords}`);
        if (modelType !== this.runningConfig.modelType) mismatches.push(`modelType: ${modelType} vs ${this.runningConfig.modelType}`);
        if (enableTimeline !== this.runningConfig.enableTimeline) mismatches.push(`enableTimeline: ${enableTimeline} vs ${this.runningConfig.enableTimeline}`);

        if (mismatches.length > 0) {
            logger.info(`[TranscriptionService:${this.instanceId}] Config mismatch detected. Model will be re-initialized.`, mismatches);
            return false;
        }

        return true;
    }

    async stop(): Promise<void> {
        if (!this.isRunning) return;
        try {
            await stopRecognizer(this.instanceId);
        } finally {
            this.isRunning = false;
        }
    }

    async softStop(): Promise<void> {
        if (this.isRunning) {
            try {
                await flushRecognizer(this.instanceId);
            } catch (error) {
                logger.error('Flush failed:', error);
            }
        }
        await this.stop();
    }

    async pauseStream(): Promise<void> {
        await this.softStop();
    }

    async resumeStream(): Promise<void> {
        if (this.isRunning) {
            return;
        }

        if (!this.modelPath) {
            const errorMessage = 'Model path not configured';
            if (this.onError) this.onError(errorMessage);
            throw new Error(errorMessage);
        }

        const registration = TranscriptionService.instanceCallbacks.get(this.instanceId);
        if (!registration) {
            throw new Error(`No active callback registration for ${this.instanceId}`);
        }

        await TranscriptionService.ensureGlobalBusFor(this.instanceId);

        if (!this._isConfigMatch()) {
            await this._initBackend();
        }

        await this._startStream();
    }

    async terminate(): Promise<void> {
        await this.stop();
    }

    async sendAudioInt16(samples: Int16Array): Promise<void> {
        if (!this.isRunning) return;
        try {
            const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
            await feedAudioChunk(this.instanceId, bytes);
        } catch (error) {
            logger.error('Feed audio failed:', error);
        }
    }

    async transcribeFile(
        filePath: string,
        onProgress?: (progress: number) => void,
        onSegment?: TranscriptionCallback,
        language?: string,
        saveToPath?: string,
        configOverride?: AppConfig,
    ): Promise<TranscriptSegment[]> {
        try {
            return await this._transcribeFileInternal(
                filePath,
                undefined,
                onProgress,
                onSegment,
                language,
                saveToPath,
                configOverride,
            );
        } catch (error) {
            if (extractErrorMessage(error).includes('COREML_FAILURE')) {
                return await this._transcribeFileInternal(
                    filePath,
                    'cpu',
                    onProgress,
                    onSegment,
                    language,
                    saveToPath,
                    configOverride,
                );
            }
            throw error;
        }
    }

    private async _transcribeFileInternal(
        filePath: string,
        _provider?: string,
        onProgress?: (progress: number) => void,
        onSegment?: TranscriptionCallback,
        language?: string,
        _saveToPath?: string,
        configOverride?: AppConfig,
    ): Promise<TranscriptSegment[]> {
        if (!this.modelPath) throw new Error('Model path not configured');

        const appConfig = configOverride || useTranscriptStore.getState().config;
        let punctuationPathToUse = '';
        let vadPathToUse = '';
        let vadBufferToUse = 5.0;

        const offlineModel = findSelectedModelByMode(this.modelPath, 'offline');
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

        const segments = await processBatchFile({
            filePath, saveToPath: _saveToPath || null, modelPath: this.modelPath, numThreads: 4, enableItn: this.enableITN,
            language: language || this.language || 'auto', punctuationModel: punctuationPathToUse || null,
            vadModel: vadPathToUse || null, vadBuffer: vadBufferToUse, modelType: offlineModel?.type || 'sensevoice',
            fileConfig: offlineModel?.fileConfig,
            hotwords: hotwordsStr,
            speakerProcessing: speakerService.buildProcessingConfig(appConfig),
            normalizationOptions: {
                enableTimeline: appConfig.enableTimeline ?? false,
            },
        });

        // Filter segments: some models (like Whisper) occasionally produce single "." segments
        const filteredSegments = normalizeTranscriptSegments(
            segments.filter(seg => !(seg.text === '.' && seg.isFinal)),
        );
        
        // Apply text replacements
        const processedSegments = normalizeTranscriptSegments(filteredSegments.map(seg => ({
            ...seg,
            text: applyTextReplacements(seg.text, appConfig.textReplacementSets)
        })));

        if (onProgress) onProgress(100);
        if (onSegment) processedSegments.forEach(seg => onSegment(seg));
        return processedSegments;
    }
}

export const transcriptionService = new TranscriptionService('record');
export const captionTranscriptionService = new TranscriptionService('caption');
