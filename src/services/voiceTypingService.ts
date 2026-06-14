import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useConfigStore } from '../stores/configStore';
import { useVoiceTypingRuntimeStore } from '../stores/voiceTypingRuntimeStore';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import { isAsrRequestConfigured } from './asrConfigService';
import { createTranscriptionService, TranscriptionService } from './transcriptionService';
import {
    getMousePosition,
    getTextCursorPosition,
    injectText,
} from './tauri/system';
import { VoiceTypingMicrophoneRuntime } from './voiceTyping/voiceTypingMicrophoneRuntime';
import { VoiceTypingOverlayPresenter } from './voiceTyping/voiceTypingOverlayPresenter';
import { VoiceTypingSessionMachine } from './voiceTyping/voiceTypingSessionMachine';
import { VoiceTypingShortcutController } from './voiceTyping/voiceTypingShortcutController';
import {
    getVoiceTypingShortcutModifiers,
    resolveVoiceTypingAsr,
    resolveVoiceTypingConfigSnapshot,
    resolveVoiceTypingRuntimeChange,
    type VoiceTypingConfigSnapshot,
    type VoiceTypingShortcutModifier,
} from './voiceTyping/voiceTypingConfig';
import { initRecognizer, processBatchFile } from './tauri/recognizer';
import type { AppConfig } from '../types/config';

const CURSOR_POSITION_OFFSET = 12;
const MOUSE_POSITION_OFFSET = 20;
const POST_COMMIT_CARET_RETRY_DELAYS_MS = [0, 40, 40, 40];

export interface VoiceTypingServicePorts {
    getConfig: () => AppConfig;
    subscribeConfig: typeof useConfigStore.subscribe;
    getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
    getVoiceTypingRuntimeStore: typeof useVoiceTypingRuntimeStore.getState;
    injectText: typeof injectText;
    getTextCursorPosition: typeof getTextCursorPosition;
    getMousePosition: typeof getMousePosition;
    transcriptionService: TranscriptionService;
}

export class VoiceTypingService {
    private initialized = false;

    private lastConfigSnapshot: VoiceTypingConfigSnapshot | null = null;

    private readonly transcriptionService: TranscriptionService;
    private readonly overlayPresenter = new VoiceTypingOverlayPresenter();
    private readonly microphoneRuntime = new VoiceTypingMicrophoneRuntime();
    private readonly sessionMachine: VoiceTypingSessionMachine;
    private readonly shortcutController: VoiceTypingShortcutController;

    constructor(private readonly ports: VoiceTypingServicePorts) {
        this.transcriptionService = ports.transcriptionService;
        this.sessionMachine = new VoiceTypingSessionMachine({
            transcriptionService: this.transcriptionService,
            overlayPresenter: this.overlayPresenter,
            resolveOverlayPosition: () => this.getOverlayPosition(),
            resolveOverlayPositionAfterCommit: () => this.getOverlayPositionAfterCommit(),
            ensureMicrophoneStarted: () => this.ensureMicrophoneStarted(),
            injectText: async (text) => {
                const shortcutModifiers = this.getCurrentShortcutModifiers();
                await this.ports.injectText(text, shortcutModifiers);
            },
            onRuntimeError: (error) => {
                this.ports.getVoiceTypingRuntimeStore().reportRuntimeError('session', error);
            },
        });
        this.shortcutController = new VoiceTypingShortcutController({
            getMode: () => this.getVoiceTypingMode(),
            isListening: () => this.sessionMachine.isActive(),
            startListening: () => this.startListening(),
            stopListening: () => this.stopListening(),
        });
    }

    public init() {
        if (this.initialized) {
            logger.info('[VoiceTypingService] Already initialized.');
            return;
        }

        this.initialized = true;
        logger.info('[VoiceTypingService] Initializing...');

        const initialConfig = this.ports.getConfig();
        this.lastConfigSnapshot = resolveVoiceTypingConfigSnapshot(initialConfig);

        logger.info('[VoiceTypingService] Initial config', {
            enabled: this.lastConfigSnapshot.enabled,
            shortcut: this.lastConfigSnapshot.shortcut,
            asr: this.lastConfigSnapshot.asrSignature,
            vadModelPath: this.lastConfigSnapshot.vadModelPath,
            microphoneId: this.lastConfigSnapshot.microphoneId,
            keepMicrophoneActive: this.lastConfigSnapshot.keepMicrophoneActive,
            language: this.lastConfigSnapshot.language,
            enableITN: this.lastConfigSnapshot.enableItn,
        });

        this.ports.subscribeConfig((state) => {
            const newConfig = state.config;
            const previousSnapshot = this.lastConfigSnapshot ?? resolveVoiceTypingConfigSnapshot(newConfig);
            const nextSnapshot = resolveVoiceTypingConfigSnapshot(newConfig);
            const change = resolveVoiceTypingRuntimeChange(previousSnapshot, nextSnapshot);

            if (!nextSnapshot.enabled) {
                this.ports.getVoiceTypingRuntimeStore().resetRuntimeStatus();
            } else if (change.enabledChanged) {
                this.ports.getVoiceTypingRuntimeStore().clearRuntimeFailure({
                    resetShortcutRegistration: true,
                    resetWarmup: true,
                });
            } else if (change.runtimeDependencyChanged) {
                this.ports.getVoiceTypingRuntimeStore().clearRuntimeFailure({
                    resetShortcutRegistration: change.shortcutChanged,
                    resetWarmup:
                        change.asrChanged ||
                        change.vadModelChanged ||
                        change.microphoneChanged ||
                        change.keepMicrophoneActiveChanged,
                });
            }

            if (change.enabledChanged || change.shortcutChanged) {
                logger.info('[VoiceTypingService] Shortcut config changed', {
                    enabled: nextSnapshot.enabled,
                    shortcut: nextSnapshot.shortcut,
                });
                void this.updateShortcutRegistration(nextSnapshot.enabled, nextSnapshot.shortcut);

                if (!nextSnapshot.enabled) {
                    void this.stopMicrophoneCapture();
                }
            }

            if (
                change.keepMicrophoneActiveChanged &&
                !nextSnapshot.keepMicrophoneActive &&
                !this.sessionMachine.isActive()
            ) {
                void this.stopMicrophoneCapture();
            }

            this.lastConfigSnapshot = nextSnapshot;

            if (nextSnapshot.enabled && (change.configChanged || change.enabledChanged)) {
                void this.syncAndPrepare();
            }
        });

        void this.updateShortcutRegistration(this.lastConfigSnapshot.enabled, this.lastConfigSnapshot.shortcut);
        if (this.lastConfigSnapshot.enabled) {
            void this.syncAndPrepare();
        }
    }

    private async syncAndPrepare() {
        const config = this.ports.getConfig();
        const asr = resolveVoiceTypingAsr(this.ports.getEffectiveConfigSnapshot());
        if (!config.voiceTypingEnabled || !isAsrRequestConfigured(asr)) {
            this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('idle');
            return;
        }

        try {
            this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('preparing');
            logger.info('[VoiceTypingService] Pre-warming transcription model and microphone...');
            this.configureTranscriptionService();
            await this.transcriptionService.prepare();

            const lastPosition = this.sessionMachine.getLastPosition() ?? [0, 0];
            await this.overlayPresenter.prepare(lastPosition);
            if (config.keepMicrophoneActive ?? false) {
                await this.ensureMicrophoneStarted();
            }

            if (this.ports.getVoiceTypingRuntimeStore().warmup === 'error') {
                return;
            }

            this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('ready');

            logger.info('[VoiceTypingService] Model and mic pre-warmed and ready.');
        } catch (error) {
            logger.error('[VoiceTypingService] Failed to pre-warm:', error);
            this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('error', {
                errorSource: 'warmup',
                errorMessage: extractErrorMessage(error),
            });
        }
    }

    private configureTranscriptionService() {
        const asr = resolveVoiceTypingAsr(this.ports.getEffectiveConfigSnapshot());
        if (asr.engine === 'local-sherpa') {
            this.transcriptionService.setModelPath(asr.modelPath);
        }
        this.transcriptionService.setLanguage(asr.language);
        this.transcriptionService.setEnableITN(asr.enableItn);
    }

    private async ensureMicrophoneStarted() {
        const config = this.ports.getConfig();
        await this.microphoneRuntime.ensureStarted(config.microphoneId);
    }

    private async stopMicrophoneCapture() {
        await this.microphoneRuntime.stop();
    }

    private async updateShortcutRegistration(enabled: boolean, shortcut: string) {
        await this.shortcutController.update(enabled, shortcut);
    }

    private async startListening() {
        this.configureTranscriptionService();
        await this.sessionMachine.start();
    }

    private async stopListening() {
        await this.sessionMachine.stop();
        if (!(this.ports.getConfig().keepMicrophoneActive ?? false)) {
            await this.stopMicrophoneCapture();
        }
    }

    private getVoiceTypingMode() {
        return this.ports.getConfig().voiceTypingMode || 'hold';
    }

    private getCurrentShortcutModifiers(): VoiceTypingShortcutModifier[] {
        const shortcut = this.ports.getConfig().voiceTypingShortcut ?? 'Alt+V';
        return getVoiceTypingShortcutModifiers(shortcut);
    }

    private normalizeOverlayPosition(cursorPosition: [number, number]): [number, number] {
        const marginCompensation = 4;
        return [
            cursorPosition[0] - marginCompensation,
            cursorPosition[1] + CURSOR_POSITION_OFFSET - marginCompensation,
        ];
    }

    private async delay(ms: number) {
        await new Promise<void>((resolve) => {
            setTimeout(resolve, ms);
        });
    }

    private async tryGetTextCursorOverlayPosition() {
        try {
            const cursorPosition = await this.ports.getTextCursorPosition();
            if (!cursorPosition) {
                return null;
            }

            return this.normalizeOverlayPosition(cursorPosition);
        } catch (error) {
            logger.debug(
                '[VoiceTypingService] Failed to get text cursor position, falling back to mouse.',
                error
            );
            return null;
        }
    }

    private async getOverlayPosition(): Promise<[number, number]> {
        const cursorPosition = await this.tryGetTextCursorOverlayPosition();
        if (cursorPosition) {
            return cursorPosition;
        }

        const lastPosition = this.sessionMachine.getLastPosition();
        if (lastPosition) {
            return lastPosition;
        }

        const [x, y] = await this.ports.getMousePosition();
        return [x - 4, y + MOUSE_POSITION_OFFSET - 4];
    }

    private async getOverlayPositionAfterCommit(): Promise<[number, number]> {
        const previousPosition = this.sessionMachine.getLastPosition();
        let latestCursorPosition: [number, number] | null = null;

        for (let attempt = 0; attempt < POST_COMMIT_CARET_RETRY_DELAYS_MS.length; attempt += 1) {
            const retryDelay = POST_COMMIT_CARET_RETRY_DELAYS_MS[attempt];
            if (retryDelay > 0) {
                await this.delay(retryDelay);
            }

            const cursorPosition = await this.tryGetTextCursorOverlayPosition();
            const moved =
                !!cursorPosition &&
                (!previousPosition ||
                    cursorPosition[0] !== previousPosition[0] ||
                    cursorPosition[1] !== previousPosition[1]);

            logger.info('[VoiceTypingService] Post-commit caret probe', {
                attempt: attempt + 1,
                retryDelay,
                previousPosition,
                nextPosition: cursorPosition,
                repositionAttempt: true,
                repositionMoved: moved,
            });

            if (!cursorPosition) {
                continue;
            }

            latestCursorPosition = cursorPosition;
            if (moved) {
                return cursorPosition;
            }
        }

        if (previousPosition) {
            logger.info('[VoiceTypingService] Falling back to last overlay position after commit', {
                previousPosition,
                latestCursorPosition,
                repositionAttempt: true,
                repositionMoved: false,
            });
            return previousPosition;
        }

        if (latestCursorPosition) {
            return latestCursorPosition;
        }

        return await this.getOverlayPosition();
    }

    async retryWarmup() {
        if (!this.initialized) {
            this.init();
            return;
        }

        this.ports.getVoiceTypingRuntimeStore().clearRuntimeFailure({
            resetWarmup: true,
        });
        await this.stopMicrophoneCapture();
        await this.syncAndPrepare();
    }

    resetForTest() {
        this.initialized = false;
        this.lastConfigSnapshot = null;
        this.microphoneRuntime.resetForTest();
        this.shortcutController.resetForTest();
        this.overlayPresenter.resetForTest();
        this.sessionMachine.resetForTest();
        this.ports.getVoiceTypingRuntimeStore().resetRuntimeStatus();
    }
}

export function createVoiceTypingService(ports: VoiceTypingServicePorts): VoiceTypingService {
    return new VoiceTypingService(ports);
}

const voiceTypingTranscriptionService = createTranscriptionService('voice-typing', {
    getEffectiveConfigSnapshot,
    initRecognizer,
    processBatchFile,
});

export const voiceTypingService = createVoiceTypingService({
    getConfig: () => useConfigStore.getState().config,
    subscribeConfig: useConfigStore.subscribe,
    getEffectiveConfigSnapshot,
    getVoiceTypingRuntimeStore: useVoiceTypingRuntimeStore.getState,
    injectText,
    getTextCursorPosition,
    getMousePosition,
    transcriptionService: voiceTypingTranscriptionService,
});
