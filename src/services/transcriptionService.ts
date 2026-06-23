import { logger } from "../utils/logger";
import { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import type { AppConfig } from '../types/config';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { extractErrorMessage } from '../utils/errorUtils';
import { normalizeTranscriptSegments } from '../utils/transcriptTiming';
import {
    type AsrTranscriptionRequest,
    type TranscriptPostprocessOptions,
} from './asrConfigService';
import {
    initRecognizer,
    processBatchFile,
} from './tauri/recognizer';
import { RecognizerLifecycle } from './transcription/recognizerLifecycle';
import {
    buildBatchTranscriptionRequest,
    buildRecognizerInitRequest,
    buildStreamingAsrRequest,
    isTranscriptionRequestConfigured,
} from './transcription/transcriptionRequest';

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



export interface TranscriptionServicePorts {
    getEffectiveConfigSnapshot: () => AppConfig;
    initRecognizer: typeof initRecognizer;
    processBatchFile: typeof processBatchFile;
}

/**
 * Service to manage the transcription process via the Rust backend.
 * Uses a Global Bus pattern for event reliability.
 */
export class TranscriptionService {
    private modelPath: string = '';
    private enableITN: boolean = true;
    private onError: ErrorCallback | null = null;
    private startingPromise: Promise<void> | null = null;
    private runningConfig: ServiceConfig | null = null;
    private language: string = 'auto';
    private readonly lifecycle: RecognizerLifecycle;

    constructor(
        private readonly instanceId: string = 'default',
        private readonly ports: TranscriptionServicePorts
    ) {
        this.lifecycle = new RecognizerLifecycle(instanceId);
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

        this.lifecycle.registerCallback(onUpdate, onError, {
            owner: options?.callbackOwner,
            sessionId: options?.callbackSessionId,
        });
        await this.lifecycle.ensureGlobalBus();

        if (!this._isConfigMatch()) {
            await this._initBackend();
        }

        await this.lifecycle.start(onError);
    }

    private _buildStreamingServiceConfig(): ServiceConfig {
        const appConfig = this.ports.getEffectiveConfigSnapshot();
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
                    appConfig: this.ports.getEffectiveConfigSnapshot(),
                    instanceId: this.instanceId,
                    modelPathOverride: this.modelPath,
                    language: this.language,
                    enableItn: this.enableITN,
                });
                if (!isTranscriptionRequestConfigured(configToUse)) {
                    throw new Error('ASR is not configured');
                }

                await this.ports.initRecognizer(request);

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

    private _isConfigMatch(): boolean {
        if (!this.runningConfig) return false;

        const nextRequest = this._buildStreamingServiceConfig();

        const mismatches: string[] = [];
        if (nextRequest.engine !== this.runningConfig.engine) mismatches.push(`engine: ${nextRequest.engine} vs ${this.runningConfig.engine}`);
        if (this.enableITN !== this.runningConfig.enableItn) mismatches.push(`enableITN: ${this.enableITN} vs ${this.runningConfig.enableItn}`);
        if (this.language !== this.runningConfig.language) mismatches.push(`language: ${this.language} vs ${this.runningConfig.language}`);
        if (nextRequest.normalizationOptions.enableTimeline !== this.runningConfig.normalizationOptions.enableTimeline) mismatches.push(`enableTimeline: ${nextRequest.normalizationOptions.enableTimeline} vs ${this.runningConfig.normalizationOptions.enableTimeline}`);
        if (!arePostprocessOptionsEqual(nextRequest.postprocessOptions, this.runningConfig.postprocessOptions)) mismatches.push('postprocessOptions changed');
        
        if (nextRequest.engine === 'local-sherpa' && this.runningConfig.engine === 'local-sherpa') {
            if (nextRequest.modelPath !== this.runningConfig.modelPath) mismatches.push(`modelPath: ${nextRequest.modelPath} vs ${this.runningConfig.modelPath}`);
            if (nextRequest.vadModel !== this.runningConfig.vadModel) mismatches.push(`vadModel: ${nextRequest.vadModel} vs ${this.runningConfig.vadModel}`);
            if (nextRequest.punctuationModel !== this.runningConfig.punctuationModel) mismatches.push(`punctuationModel: ${nextRequest.punctuationModel} vs ${this.runningConfig.punctuationModel}`);
            if (nextRequest.hotwords !== this.runningConfig.hotwords) mismatches.push(`hotwords: ${nextRequest.hotwords} vs ${this.runningConfig.hotwords}`);
            if (nextRequest.modelType !== this.runningConfig.modelType) mismatches.push(`modelType: ${nextRequest.modelType} vs ${this.runningConfig.modelType}`);
        } else if (nextRequest.engine === 'online' && this.runningConfig.engine === 'online') {
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
        await this.lifecycle.stop();
        this.runningConfig = null;
    }

    async softStop(): Promise<void> {
        await this.lifecycle.flushAndStop();
        this.runningConfig = null;
    }

    async pauseStream(): Promise<void> {
        await this.softStop();
    }

    async resumeStream(): Promise<void> {
        if (this.lifecycle.running) {
            return;
        }

        const nextConfig = this._buildStreamingServiceConfig();
        if (!isTranscriptionRequestConfigured(nextConfig)) {
            const errorMessage = 'ASR is not configured';
            if (this.onError) this.onError(errorMessage);
            throw new Error(errorMessage);
        }

        if (!this.lifecycle.hasCallbackRegistration()) {
            throw new Error(`No active callback registration for ${this.instanceId}`);
        }

        await this.lifecycle.ensureGlobalBus();

        if (!this._isConfigMatch()) {
            await this._initBackend();
        }

        await this.lifecycle.start((error) => this.onError?.(error));
    }

    async terminate(): Promise<void> {
        await this.stop();
    }

    async sendAudioInt16(samples: Int16Array): Promise<void> {
        await this.lifecycle.feedAudioInt16(samples);
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
        const appConfig = configOverride || this.ports.getEffectiveConfigSnapshot();
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

        const segments = await this.ports.processBatchFile(batchRequest);

        const processedSegments = normalizeTranscriptSegments(segments);

        if (onProgress) onProgress(100);
        if (onSegment) processedSegments.forEach(seg => onSegment(seg));
        return processedSegments;
    }
}

export function createTranscriptionService(
    instanceId: string,
    ports: TranscriptionServicePorts
): TranscriptionService {
    return new TranscriptionService(instanceId, ports);
}

const defaultPorts: TranscriptionServicePorts = {
    getEffectiveConfigSnapshot,
    initRecognizer,
    processBatchFile,
};

export const transcriptionService = createTranscriptionService('record', defaultPorts);
export const captionTranscriptionService = createTranscriptionService('caption', defaultPorts);
