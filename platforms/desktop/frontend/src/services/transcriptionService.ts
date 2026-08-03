import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import type { AppConfig } from '../types/config';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { extractErrorMessage } from '../utils/errorUtils';
import { normalizeTranscriptSegments } from '../utils/transcriptTiming';
import { type AsrTranscriptionRequest } from './asrConfigService';
import {
    pauseNativeLiveTranscription,
    prepareLiveTranscription,
    processBatchFile,
    resumeNativeLiveTranscription,
    startNativeLiveTranscription,
    stopNativeLiveTranscription,
} from './tauri/recognizer';
import { RecognizerLifecycle } from './transcription/recognizerLifecycle';
import {
    buildBatchTranscriptionRequest,
    buildStreamingAsrRequest,
    isTranscriptionRequestConfigured,
} from './transcription/transcriptionRequest';
import { buildRecognizerOutputEvent } from './tauri/events';
import { listen, type UnlistenFn } from './tauri/platform/events';

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

export interface NativeTranscriptionStartOptions extends StartOptions {
    sourceKind: 'system' | 'microphone';
    deviceName: string | null;
    outputPath?: string | null;
    gain?: number;
}

type ServiceConfig = AsrTranscriptionRequest;

export interface TranscriptionServicePorts {
    getEffectiveConfigSnapshot: () => AppConfig;
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
    private runningConfig: ServiceConfig | null = null;
    private language: string = 'auto';
    private readonly lifecycle: RecognizerLifecycle;
    private transport: 'idle' | 'external' | 'native' = 'idle';
    private nativeSourceKind: 'system' | 'microphone' | null = null;
    private nativeGain = 1;
    private preparedNativeConfig: ServiceConfig | null = null;

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
        const config = this._buildStreamingServiceConfig();
        if (!isTranscriptionRequestConfigured(config)) return;
        await prepareLiveTranscription(config);
    }

    async startExternal(
        onUpdate: TranscriptionUpdateCallback,
        onError: ErrorCallback,
        options?: StartOptions,
    ): Promise<void> {
        const config = await this.prepareSharedStart(onUpdate, onError, options);
        await prepareLiveTranscription(config);
        await this.lifecycle.startExternal(config, onError);
        this.runningConfig = config;
        this.transport = 'external';
        this.nativeSourceKind = null;
    }

    async startNative(
        onUpdate: TranscriptionUpdateCallback,
        onError: ErrorCallback,
        options: NativeTranscriptionStartOptions,
    ): Promise<void> {
        await this.prepareNativeStart(onUpdate, onError, options);
        await this.attachPreparedNative(options);
    }

    async prepareNativeStart(
        onUpdate: TranscriptionUpdateCallback,
        onError: ErrorCallback,
        options?: StartOptions,
    ): Promise<void> {
        const config = await this.prepareSharedStart(onUpdate, onError, options);
        await prepareLiveTranscription(config);
        this.preparedNativeConfig = config;
    }

    async attachPreparedNative(options: NativeTranscriptionStartOptions): Promise<void> {
        const config = this.preparedNativeConfig;
        if (!config) {
            throw new Error(`Native transcription for ${this.instanceId} was not prepared`);
        }
        await startNativeLiveTranscription({
            consumerId: this.instanceId,
            sourceKind: options.sourceKind,
            deviceName: options.deviceName,
            outputPath: options.outputPath ?? null,
            gain: options.gain ?? 1,
            asrRequest: config,
        });
        this.lifecycle.markNativeRunning();
        this.runningConfig = config;
        this.transport = 'native';
        this.nativeSourceKind = options.sourceKind;
        this.nativeGain = options.gain ?? 1;
        this.preparedNativeConfig = null;
    }

    private async prepareSharedStart(
        onUpdate: TranscriptionUpdateCallback,
        onError: ErrorCallback,
        options?: StartOptions,
    ): Promise<ServiceConfig> {
        this.onError = onError;
        const config = this._buildStreamingServiceConfig();
        if (!isTranscriptionRequestConfigured(config)) {
            const errorMessage = 'ASR is not configured';
            onError(errorMessage);
            throw new Error(errorMessage);
        }
        this.lifecycle.registerCallback(onUpdate, onError, {
            owner: options?.callbackOwner,
            sessionId: options?.callbackSessionId,
        });
        await this.lifecycle.ensureGlobalBus();
        return config;
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

    async stop(): Promise<void> {
        this.preparedNativeConfig = null;
        if (this.transport === 'external') {
            await this.lifecycle.stopExternal();
            this.runningConfig = null;
            this.transport = 'idle';
            return;
        }
        if (this.transport === 'native' && this.nativeSourceKind) {
            await stopNativeLiveTranscription(this.instanceId, this.nativeSourceKind);
            this.lifecycle.markStopped();
            this.runningConfig = null;
            this.transport = 'idle';
            this.nativeSourceKind = null;
            return;
        }
        this.runningConfig = null;
        this.transport = 'idle';
    }

    async stopNativeCapture(): Promise<string> {
        this.preparedNativeConfig = null;
        if (this.transport !== 'native' || !this.nativeSourceKind) {
            return '';
        }
        const path = await stopNativeLiveTranscription(this.instanceId, this.nativeSourceKind);
        this.lifecycle.markStopped();
        this.runningConfig = null;
        this.transport = 'idle';
        this.nativeSourceKind = null;
        return path;
    }

    async softStop(): Promise<void> {
        this.preparedNativeConfig = null;
        if (this.transport === 'native' && this.nativeSourceKind) {
            await pauseNativeLiveTranscription(this.instanceId, this.nativeSourceKind);
            this.lifecycle.markStopped();
            return;
        }
        if (this.transport === 'external') {
            await this.lifecycle.stopExternal();
            return;
        }
        this.runningConfig = null;
        this.transport = 'idle';
    }

    async pauseStream(): Promise<void> {
        if (this.transport === 'native' && this.nativeSourceKind) {
            await pauseNativeLiveTranscription(this.instanceId, this.nativeSourceKind);
            this.lifecycle.markStopped();
            return;
        }
        if (this.transport === 'external') {
            await this.lifecycle.stopExternal();
            return;
        }
    }

    async resumeStream(): Promise<void> {
        if (this.transport === 'native' && this.nativeSourceKind && this.runningConfig) {
            await resumeNativeLiveTranscription({
                consumerId: this.instanceId,
                sourceKind: this.nativeSourceKind,
                gain: this.nativeGain,
                asrRequest: this.runningConfig,
            });
            this.lifecycle.markNativeRunning();
            return;
        }
        if (this.transport === 'external' && this.runningConfig) {
            await this.lifecycle.startExternal(
                this.runningConfig,
                (error) => this.onError?.(error),
            );
            return;
        }
    }

    async restartStream(): Promise<void> {
        const config = this._buildStreamingServiceConfig();
        if (!isTranscriptionRequestConfigured(config)) {
            throw new Error('ASR is not configured');
        }
        await prepareLiveTranscription(config);
        if (this.transport === 'native' && this.nativeSourceKind) {
            await pauseNativeLiveTranscription(this.instanceId, this.nativeSourceKind);
            this.lifecycle.markStopped();
            this.runningConfig = config;
            await resumeNativeLiveTranscription({
                consumerId: this.instanceId,
                sourceKind: this.nativeSourceKind,
                gain: this.nativeGain,
                asrRequest: config,
            });
            this.lifecycle.markNativeRunning();
            return;
        }
        if (this.transport === 'external') {
            await this.lifecycle.stopExternal();
            await this.lifecycle.startExternal(config, (error) => this.onError?.(error));
            this.runningConfig = config;
        }
    }

    async terminate(): Promise<void> {
        await this.stop();
    }

    async sendAudioInt16(samples: Int16Array): Promise<void> {
        if (this.transport === 'external') {
            await this.lifecycle.feedExternalAudioInt16(samples);
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
        const appConfig = configOverride || this.ports.getEffectiveConfigSnapshot();
        const instanceId = `batch-${uuidv4()}`;
        const { request: batchRequest, asrRequest } = buildBatchTranscriptionRequest({
            appConfig,
            filePath,
            saveToPath: _saveToPath || null,
            modelPathOverride: this.modelPath,
            language: language || this.language || 'auto',
            enableItn: this.enableITN,
            instanceId,
        });
        if (!isTranscriptionRequestConfigured(asrRequest)) {
            throw new Error('ASR is not configured');
        }

        let unlisten: UnlistenFn | undefined;
        if (onSegment) {
            unlisten = await listen<TranscriptUpdate>(
                buildRecognizerOutputEvent(instanceId),
                (event) => {
                    for (const segment of event.payload.upsertSegments) {
                        onSegment(segment);
                    }
                },
            );
        }

        try {
            const segments = await this.ports.processBatchFile(batchRequest);

            const processedSegments = normalizeTranscriptSegments(segments);

            if (onProgress) onProgress(100);
            return processedSegments;
        } finally {
            unlisten?.();
        }
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
    processBatchFile,
};

export const transcriptionService = createTranscriptionService('record', defaultPorts);
export const captionTranscriptionService = createTranscriptionService('caption', defaultPorts);
