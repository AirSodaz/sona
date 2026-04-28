import { invoke } from '@tauri-apps/api/core';
import { isRegistered, register, unregister } from '@tauri-apps/plugin-global-shortcut';
import { useConfigStore } from '../stores/configStore';
import { useVoiceTypingRuntimeStore } from '../stores/voiceTypingRuntimeStore';
import { logger } from '../utils/logger';
import { TranscriptionService } from './transcriptionService';
import { VoiceTypingOverlayPresenter } from './voiceTyping/voiceTypingOverlayPresenter';
import { VoiceTypingSessionMachine } from './voiceTyping/voiceTypingSessionMachine';

const CURSOR_POSITION_OFFSET = 12;
const MOUSE_POSITION_OFFSET = 20;
const POST_COMMIT_CARET_RETRY_DELAYS_MS = [0, 40, 40, 40];
type VoiceTypingShortcutModifier = 'control' | 'alt' | 'shift' | 'meta';

function normalizeErrorMessage(error: unknown) {
    if (error instanceof Error && error.message) {
        return error.message;
    }

    return String(error);
}

class VoiceTypingService {
    private initialized = false;
    private isShortcutRegistered = false;
    private currentShortcut: string | null = null;
    private captureStarted = false;

    private lastEnabled = false;
    private lastShortcut = '';
    private lastModelPath = '';
    private lastVadModelPath = '';
    private lastMicrophoneId = 'default';
    private lastLanguage = '';
    private lastEnableITN = true;

    private readonly transcriptionService = new TranscriptionService('voice-typing');
    private readonly overlayPresenter = new VoiceTypingOverlayPresenter();
    private readonly sessionMachine = new VoiceTypingSessionMachine({
        transcriptionService: this.transcriptionService,
        overlayPresenter: this.overlayPresenter,
        resolveOverlayPosition: () => this.getOverlayPosition(),
        resolveOverlayPositionAfterCommit: () => this.getOverlayPositionAfterCommit(),
        ensureMicrophoneStarted: () => this.ensureMicrophoneStarted(),
        injectText: async (text) => {
            const shortcutModifiers = this.getCurrentShortcutModifiers();
            await invoke('inject_text', shortcutModifiers.length > 0 ? {
                text,
                shortcutModifiers,
            } : {
                text,
            });
        },
        onRuntimeError: (error) => {
            useVoiceTypingRuntimeStore.getState().reportRuntimeError('session', error);
        },
    });

    public init() {
        if (this.initialized) {
            logger.info('[VoiceTypingService] Already initialized.');
            return;
        }

        this.initialized = true;
        logger.info('[VoiceTypingService] Initializing...');

        const initialConfig = useConfigStore.getState().config;
        this.lastEnabled = initialConfig.voiceTypingEnabled || false;
        this.lastShortcut = initialConfig.voiceTypingShortcut ?? 'Alt+V';
        this.lastModelPath = initialConfig.streamingModelPath || '';
        this.lastVadModelPath = initialConfig.vadModelPath || '';
        this.lastMicrophoneId = initialConfig.microphoneId || 'default';
        this.lastLanguage = initialConfig.language || 'auto';
        this.lastEnableITN = initialConfig.enableITN ?? true;

        logger.info('[VoiceTypingService] Initial config', {
            enabled: this.lastEnabled,
            shortcut: this.lastShortcut,
            modelPath: this.lastModelPath,
            vadModelPath: this.lastVadModelPath,
            microphoneId: this.lastMicrophoneId,
            language: this.lastLanguage,
            enableITN: this.lastEnableITN,
        });

        useConfigStore.subscribe((state) => {
            const newConfig = state.config;
            const newEnabled = newConfig.voiceTypingEnabled || false;
            const newShortcut = newConfig.voiceTypingShortcut ?? 'Alt+V';
            const newModelPath = newConfig.streamingModelPath || '';
            const newVadModelPath = newConfig.vadModelPath || '';
            const newMicrophoneId = newConfig.microphoneId || 'default';
            const newLanguage = newConfig.language || 'auto';
            const newEnableITN = newConfig.enableITN ?? true;

            const enabledChanged = newEnabled !== this.lastEnabled;
            const shortcutChanged = newShortcut !== this.lastShortcut;
            const vadModelChanged = newVadModelPath !== this.lastVadModelPath;
            const microphoneChanged = newMicrophoneId !== this.lastMicrophoneId;
            const configChanged =
                newModelPath !== this.lastModelPath ||
                vadModelChanged ||
                microphoneChanged ||
                newLanguage !== this.lastLanguage ||
                newEnableITN !== this.lastEnableITN;
            const runtimeDependencyChanged =
                shortcutChanged ||
                newModelPath !== this.lastModelPath ||
                vadModelChanged ||
                microphoneChanged;

            if (!newEnabled) {
                useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
            } else if (enabledChanged) {
                useVoiceTypingRuntimeStore.getState().clearRuntimeFailure({
                    resetShortcutRegistration: true,
                    resetWarmup: true,
                });
            } else if (runtimeDependencyChanged) {
                useVoiceTypingRuntimeStore.getState().clearRuntimeFailure({
                    resetShortcutRegistration: shortcutChanged,
                    resetWarmup:
                        newModelPath !== this.lastModelPath ||
                        vadModelChanged ||
                        microphoneChanged,
                });
            }

            if (enabledChanged || shortcutChanged) {
                logger.info('[VoiceTypingService] Shortcut config changed', {
                    enabled: newEnabled,
                    shortcut: newShortcut,
                });
                void this.updateShortcutRegistration(newEnabled, newShortcut);

                if (!newEnabled) {
                    void this.stopMicrophoneCapture();
                }
            }

            this.lastEnabled = newEnabled;
            this.lastShortcut = newShortcut;
            this.lastModelPath = newModelPath;
            this.lastVadModelPath = newVadModelPath;
            this.lastMicrophoneId = newMicrophoneId;
            this.lastLanguage = newLanguage;
            this.lastEnableITN = newEnableITN;

            if (newEnabled && (configChanged || enabledChanged)) {
                void this.syncAndPrepare();
            }
        });

        void this.updateShortcutRegistration(this.lastEnabled, this.lastShortcut);
        if (this.lastEnabled) {
            void this.syncAndPrepare();
        }
    }

    private async syncAndPrepare() {
        const config = useConfigStore.getState().config;
        if (!config.voiceTypingEnabled || !config.streamingModelPath) {
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
                errorMessage: normalizeErrorMessage(error),
            });
        }
    }

    private configureTranscriptionService() {
        const config = useConfigStore.getState().config;
        this.transcriptionService.setModelPath(config.streamingModelPath);
        this.transcriptionService.setLanguage(config.language);
        this.transcriptionService.setEnableITN(config.enableITN ?? true);
    }

    private async ensureMicrophoneStarted() {
        if (this.captureStarted) {
            return;
        }

        try {
            const config = useConfigStore.getState().config;
            logger.info('[VoiceTypingService] Starting microphone capture for pre-warming...');
            await invoke('start_microphone_capture', {
                deviceName: config.microphoneId === 'default' ? null : config.microphoneId,
                instanceId: 'voice-typing',
            });
            this.captureStarted = true;
        } catch (error) {
            logger.error('[VoiceTypingService] Failed to start microphone capture:', error);
            useVoiceTypingRuntimeStore.getState().setWarmupStatus('error', {
                errorSource: 'microphone',
                errorMessage: normalizeErrorMessage(error),
            });
        }
    }

    private async stopMicrophoneCapture() {
        if (!this.captureStarted) {
            return;
        }

        try {
            logger.info('[VoiceTypingService] Stopping persistent microphone capture...');
            await invoke('stop_microphone_capture', { instanceId: 'voice-typing' });
            this.captureStarted = false;
        } catch (error) {
            logger.error('[VoiceTypingService] Failed to stop microphone capture:', error);
        }
    }

    private async updateShortcutRegistration(enabled: boolean, shortcut: string) {
        const normalizedShortcut = shortcut.replace(/\s+/g, '');
        logger.info('[VoiceTypingService] updateShortcutRegistration called', {
            enabled,
            shortcut,
            normalizedShortcut,
        });

        try {
            if (this.isShortcutRegistered && this.currentShortcut) {
                const registered = await isRegistered(this.currentShortcut);
                if (registered) {
                    await unregister(this.currentShortcut);
                }
                this.isShortcutRegistered = false;
            }

            if (!enabled || !normalizedShortcut) {
                this.currentShortcut = null;
                useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('idle');
                return;
            }

            this.currentShortcut = null;
            useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('idle');

            await register(normalizedShortcut, (event) => {
                logger.info('[VoiceTypingService] Shortcut event triggered', {
                    shortcut: event.shortcut,
                    state: event.state,
                    mode: this.getVoiceTypingMode(),
                    isListening: this.sessionMachine.isActive(),
                });

                const mode = this.getVoiceTypingMode();
                if (mode === 'hold') {
                    if (event.state === 'Pressed' && !this.sessionMachine.isActive()) {
                        void this.startListening();
                    } else if (event.state === 'Released' && this.sessionMachine.isActive()) {
                        void this.stopListening();
                    }
                    return;
                }

                if (event.state === 'Pressed') {
                    if (this.sessionMachine.isActive()) {
                        void this.stopListening();
                    } else {
                        void this.startListening();
                    }
                }
            });

            this.isShortcutRegistered = true;
            this.currentShortcut = normalizedShortcut;
            useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
            logger.info('[VoiceTypingService] Successfully registered voice typing shortcut', {
                shortcut: normalizedShortcut,
            });
        } catch (error) {
            logger.error('[VoiceTypingService] Failed to update voice typing shortcut:', error);
            useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus(
                'error',
                normalizeErrorMessage(error)
            );
        }
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
        const normalizedParts = shortcut
            .split('+')
            .map((part) => part.trim().toLowerCase())
            .filter(Boolean);
        const modifiers = new Set<VoiceTypingShortcutModifier>();

        for (const part of normalizedParts) {
            if (part === 'ctrl' || part === 'control' || part === 'cmdorctrl') {
                modifiers.add('control');
                continue;
            }

            if (part === 'alt' || part === 'option') {
                modifiers.add('alt');
                continue;
            }

            if (part === 'shift') {
                modifiers.add('shift');
                continue;
            }

            if (
                part === 'meta' ||
                part === 'cmd' ||
                part === 'command' ||
                part === 'super' ||
                part === 'win'
            ) {
                modifiers.add('meta');
            }
        }

        return Array.from(modifiers);
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
            const cursorPosition = await invoke<[number, number] | null>(
                'get_text_cursor_position'
            );
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

        const [x, y] = await invoke<[number, number]>('get_mouse_position');
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
        this.isShortcutRegistered = false;
        this.currentShortcut = null;
        this.captureStarted = false;
        this.lastEnabled = false;
        this.lastShortcut = '';
        this.lastModelPath = '';
        this.lastVadModelPath = '';
        this.lastMicrophoneId = 'default';
        this.lastLanguage = '';
        this.lastEnableITN = true;
        this.overlayPresenter.resetForTest();
        this.sessionMachine.resetForTest();
        useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
    }
}

export const voiceTypingService = new VoiceTypingService();
