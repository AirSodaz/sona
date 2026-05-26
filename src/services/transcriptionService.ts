import { logger } from "../utils/logger";
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import type { AppConfig } from '../types/config';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { extractErrorMessage } from '../utils/errorUtils';
import { normalizeTranscriptSegments, normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import {
    type AsrTranscriptionRequest,
    type TranscriptPostprocessOptions,
} from './asrConfigService';
import { buildRecognizerOutputEvent } from './tauri/events';
import {
    feedAudioChunk,
    flushRecognizer,
    initRecognizer,
    processBatchFile,
    startRecognizer,
    stopRecognizer,
} from './tauri/recognizer';
import {
    buildBatchTranscriptionRequest,
    buildRecognizerInitRequest,
    buildStreamingAsrRequest,
    isTranscriptionRequestConfigured,
} from './transcription/transcriptionRequest';

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

type ServiceConfig = AsrTranscriptionRequest;

function arePostprocessOptionsEqual(
    left: TranscriptPostprocessOptions,
    right: TranscriptPostprocessOptions,
): boolean {
    return JSON.stringify(left) === JSON.stringify(right);
}

function areServiceConfigsEqual(
    left: ServiceConfig,
    right: ServiceConfig,
): boolean {
    return JSON.stringify(left) === JSON.stringify(right);
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
                for (const segment of update.upsertSegments) {
                    if (shouldLogVoiceTypingDiagnostics(instanceId)) {
                        logger.info(
                            '[TranscriptionService:voice-typing] Prepared recognizer segment for callback',
                            {
                                instanceId,
                                registrationId: instance.registrationId,
                                segmentId: segment.id,
                                isFinal: segment.isFinal,
                                rawTextLength: segment.text.length,
                                processedTextLength: segment.text.length,
                                preview: previewTextForLog(segment.text),
                                callbackInvoked: false,
                            }
                        );
                    }
                }

                instance.onUpdate(update);
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
        if (!isTranscriptionRequestConfigured(this._buildStreamingServiceConfig())) return;
        return this._initBackend();
    }

    async start(onUpdate: TranscriptionUpdateCallback, onError: ErrorCallback, options?: StartOptions): Promise<void> {
        this.onError = onError;

        const initialConfig = this._buildStreamingServiceConfig();
        if (!isTranscriptionRequestConfigured(initialConfig)) {
            const errorMessage = 'ASR is not configured';
            onError(errorMessage);
            throw new Error(errorMessage);
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

    private _buildStreamingServiceConfig(): ServiceConfig {
        const appConfig = getEffectiveConfigSnapshot();
        return buildStreamingAsrRequest({
            appConfig,
            instanceId: this.instanceId,
            modelPathOverride: this.modelPath,
            language: this.language,
            enableItn: this.enableITN,
        });
    }

    private async _initBackend(): Promise<void> {
        if (this.startingPromise) return this.startingPromise;

        this.startingPromise = (async () => {
            try {
                const { request, asrRequest: configToUse } = buildRecognizerInitRequest({
                    appConfig: getEffectiveConfigSnapshot(),
                    instanceId: this.instanceId,
                    modelPathOverride: this.modelPath,
                    language: this.language,
                    enableItn: this.enableITN,
                });
                if (!isTranscriptionRequestConfigured(configToUse)) {
                    throw new Error('ASR is not configured');
                }

                await initRecognizer(request);

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

        const nextRequest = this._buildStreamingServiceConfig();

        const mismatches: string[] = [];
        if (nextRequest.modelPath !== this.runningConfig.modelPath) mismatches.push(`modelPath: ${nextRequest.modelPath} vs ${this.runningConfig.modelPath}`);
        if (nextRequest.engine !== this.runningConfig.engine) mismatches.push(`engine: ${nextRequest.engine} vs ${this.runningConfig.engine}`);
        if (this.enableITN !== this.runningConfig.enableItn) mismatches.push(`enableITN: ${this.enableITN} vs ${this.runningConfig.enableItn}`);
        if (this.language !== this.runningConfig.language) mismatches.push(`language: ${this.language} vs ${this.runningConfig.language}`);
        if (nextRequest.vadModel !== this.runningConfig.vadModel) mismatches.push(`vadModel: ${nextRequest.vadModel} vs ${this.runningConfig.vadModel}`);
        if (nextRequest.punctuationModel !== this.runningConfig.punctuationModel) mismatches.push(`punctuationModel: ${nextRequest.punctuationModel} vs ${this.runningConfig.punctuationModel}`);
        if (nextRequest.hotwords !== this.runningConfig.hotwords) mismatches.push(`hotwords: ${nextRequest.hotwords} vs ${this.runningConfig.hotwords}`);
        if (nextRequest.modelType !== this.runningConfig.modelType) mismatches.push(`modelType: ${nextRequest.modelType} vs ${this.runningConfig.modelType}`);
        if (nextRequest.providerId !== this.runningConfig.providerId) mismatches.push(`providerId: ${nextRequest.providerId} vs ${this.runningConfig.providerId}`);
        if (nextRequest.profileId !== this.runningConfig.profileId) mismatches.push(`profileId: ${nextRequest.profileId} vs ${this.runningConfig.profileId}`);
        if (nextRequest.normalizationOptions.enableTimeline !== this.runningConfig.normalizationOptions.enableTimeline) mismatches.push(`enableTimeline: ${nextRequest.normalizationOptions.enableTimeline} vs ${this.runningConfig.normalizationOptions.enableTimeline}`);
        if (!arePostprocessOptionsEqual(nextRequest.postprocessOptions, this.runningConfig.postprocessOptions)) mismatches.push('postprocessOptions changed');
        if (!areServiceConfigsEqual(nextRequest, { ...this.runningConfig, postprocessOptions: nextRequest.postprocessOptions })) {
            if (JSON.stringify(nextRequest.onlineProvider) !== JSON.stringify(this.runningConfig.onlineProvider)) {
                mismatches.push('online ASR provider config changed');
            }
        }

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

        const nextConfig = this._buildStreamingServiceConfig();
        if (!isTranscriptionRequestConfigured(nextConfig)) {
            const errorMessage = 'ASR is not configured';
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
        const appConfig = configOverride || getEffectiveConfigSnapshot();
        const { request: batchRequest, asrRequest } = buildBatchTranscriptionRequest({
            appConfig,
            filePath,
            saveToPath: _saveToPath || null,
            modelPathOverride: this.modelPath,
            language: language || this.language || 'auto',
            enableItn: this.enableITN,
        });
        if (!isTranscriptionRequestConfigured(asrRequest)) {
            throw new Error('ASR is not configured');
        }

        const segments = await processBatchFile(batchRequest);

        const processedSegments = normalizeTranscriptSegments(segments);

        if (onProgress) onProgress(100);
        if (onSegment) processedSegments.forEach(seg => onSegment(seg));
        return processedSegments;
    }
}

export const transcriptionService = new TranscriptionService('record');
export const captionTranscriptionService = new TranscriptionService('caption');
