import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useConfigStore } from '../stores/configStore';
import { useVoiceTypingRuntimeStore } from '../stores/voiceTypingRuntimeStore';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import { isAsrRequestConfigured } from './asrConfigService';
import { TranscriptionService } from './transcriptionService';
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

const CURSOR_POSITION_OFFSET = 12;
const MOUSE_POSITION_OFFSET = 20;
const POST_COMMIT_CARET_RETRY_DELAYS_MS = [0, 40, 40, 40];

class VoiceTypingService {
    private initialized = false;

    private lastConfigSnapshot: VoiceTypingConfigSnapshot | null = null;

    private readonly transcriptionService = new TranscriptionService('voice-typing');
    private readonly overlayPresenter = new VoiceTypingOverlayPresenter();
    private readonly microphoneRuntime = new VoiceTypingMicrophoneRuntime();
    private readonly sessionMachine = new VoiceTypingSessionMachine({
        transcriptionService: this.transcriptionService,
        overlayPresenter: this.overlayPresenter,
        resolveOverlayPosition: () => this.getOverlayPosition(),
        resolveOverlayPositionAfterCommit: () => this.getOverlayPositionAfterCommit(),
        ensureMicrophoneStarted: () => this.ensureMicrophoneStarted(),
        injectText: async (text) => {
            const shortcutModifiers = this.getCurrentShortcutModifiers();
            await injectText(text, shortcutModifiers);
        },
        onRuntimeError: (error) => {
            useVoiceTypingRuntimeStore.getState().reportRuntimeError('session', error);
        },
    });
    private readonly shortcutController = new VoiceTypingShortcutController({
        getMode: () => this.getVoiceTypingMode(),
        isListening: () => this.sessionMachine.isActive(),
        startListening: () => this.startListening(),
        stopListening: () => this.stopListening(),
    });

    public init() {
        if (this.initialized) {
            logger.info('[VoiceTypingService] Already initialized.');
            return;
        }

        this.initialized = true;
        logger.info('[VoiceTypingService] Initializing...');

        const initialConfig = useConfigStore.getState().config;
        this.lastConfigSnapshot = resolveVoiceTypingConfigSnapshot(initialConfig);

        logger.info('[VoiceTypingService] Initial config', {
            enabled: this.lastConfigSnapshot.enabled,
            shortcut: this.lastConfigSnapshot.shortcut,
            asr: this.lastConfigSnapshot.asrSignature,
            vadModelPath: this.lastConfigSnapshot.vadModelPath,
            microphoneId: this.lastConfigSnapshot.microphoneId,
            language: this.lastConfigSnapshot.language,
            enableITN: this.lastConfigSnapshot.enableItn,
        });

        useConfigStore.subscribe((state) => {
            const newConfig = state.config;
            const previousSnapshot = this.lastConfigSnapshot ?? resolveVoiceTypingConfigSnapshot(newConfig);
            const nextSnapshot = resolveVoiceTypingConfigSnapshot(newConfig);
            const change = resolveVoiceTypingRuntimeChange(previousSnapshot, nextSnapshot);

            if (!nextSnapshot.enabled) {
                useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
            } else if (change.enabledChanged) {
                useVoiceTypingRuntimeStore.getState().clearRuntimeFailure({
                    resetShortcutRegistration: true,
                    resetWarmup: true,
                });
            } else if (change.runtimeDependencyChanged) {
                useVoiceTypingRuntimeStore.getState().clearRuntimeFailure({
                    resetShortcutRegistration: change.shortcutChanged,
                    resetWarmup:
                        change.asrChanged ||
                        change.vadModelChanged ||
                        change.microphoneChanged,
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
        const config = useConfigStore.getState().config;
        const asr = resolveVoiceTypingAsr(getEffectiveConfigSnapshot());
        if (!config.voiceTypingEnabled || !isAsrRequestConfigured(asr)) {
            useVoiceTypingRuntimeStore.getState().setWarmupStatus('idle');
            return;
        }

        try {
            useVoiceTypingRuntimeStore.getState().setWarmupStatus('preparing');
            logger.info('[VoiceTypingService] Pre-warming transcription model and microphone...');
            this.configureTranscriptionService();
            await this.transcriptionService.prepare();

            const lastPosition = this.sessionMachine.getLastPosition() ?? [0, 0];
            await this.overlayPresenter.prepare(lastPosition);
            await this.ensureMicrophoneStarted();

            if (useVoiceTypingRuntimeStore.getState().warmup === 'error') {
                return;
            }

            useVoiceTypingRuntimeStore.getState().setWarmupStatus('ready');

            logger.info('[VoiceTypingService] Model and mic pre-warmed and ready.');
        } catch (error) {
            logger.error('[VoiceTypingService] Failed to pre-warm:', error);
            useVoiceTypingRuntimeStore.getState().setWarmupStatus('error', {
                errorSource: 'warmup',
                errorMessage: extractErrorMessage(error),
            });
        }
    }

    private configureTranscriptionService() {
        const asr = resolveVoiceTypingAsr(getEffectiveConfigSnapshot());
        if (asr.engine === 'local-sherpa') {
            this.transcriptionService.setModelPath(asr.modelPath);
        }
        this.transcriptionService.setLanguage(asr.language);
        this.transcriptionService.setEnableITN(asr.enableItn);
    }

    private async ensureMicrophoneStarted() {
        const config = useConfigStore.getState().config;
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
    }

    private getVoiceTypingMode() {
        return useConfigStore.getState().config.voiceTypingMode || 'hold';
    }

    private getCurrentShortcutModifiers(): VoiceTypingShortcutModifier[] {
        const shortcut = useConfigStore.getState().config.voiceTypingShortcut ?? 'Alt+V';
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
            const cursorPosition = await getTextCursorPosition();
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

        const [x, y] = await getMousePosition();
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

        useVoiceTypingRuntimeStore.getState().clearRuntimeFailure({
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
        useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
    }
}

export const voiceTypingService = new VoiceTypingService();
