import { invoke } from '@tauri-apps/api/core';
import { isRegistered, register, unregister } from '@tauri-apps/plugin-global-shortcut';
import { useConfigStore } from '../stores/configStore';
import { logger } from '../utils/logger';
import { TranscriptionService } from './transcriptionService';
import { VoiceTypingOverlayPresenter } from './voiceTyping/voiceTypingOverlayPresenter';
import { VoiceTypingSessionMachine } from './voiceTyping/voiceTypingSessionMachine';

const CURSOR_POSITION_OFFSET = 12;
const MOUSE_POSITION_OFFSET = 20;

class VoiceTypingService {
    private initialized = false;
    private isShortcutRegistered = false;
    private currentShortcut: string | null = null;
    private captureStarted = false;

    private lastEnabled = false;
    private lastShortcut = '';
    private lastModelPath = '';
    private lastLanguage = '';
    private lastEnableITN = true;

    private readonly transcriptionService = new TranscriptionService('voice-typing');
    private readonly overlayPresenter = new VoiceTypingOverlayPresenter();
    private readonly sessionMachine = new VoiceTypingSessionMachine({
        transcriptionService: this.transcriptionService,
        overlayPresenter: this.overlayPresenter,
        resolveOverlayPosition: () => this.getOverlayPosition(),
        ensureMicrophoneStarted: () => this.ensureMicrophoneStarted(),
        injectText: async (text) => {
            await invoke('inject_text', { text });
        },
        getVoiceTypingMode: () => this.getVoiceTypingMode(),
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
        this.lastShortcut = initialConfig.voiceTypingShortcut || 'Alt+V';
        this.lastModelPath = initialConfig.streamingModelPath || '';
        this.lastLanguage = initialConfig.language || 'auto';
        this.lastEnableITN = initialConfig.enableITN ?? true;

        logger.info('[VoiceTypingService] Initial config', {
            enabled: this.lastEnabled,
            shortcut: this.lastShortcut,
            modelPath: this.lastModelPath,
            language: this.lastLanguage,
            enableITN: this.lastEnableITN,
        });

        useConfigStore.subscribe((state) => {
            const newConfig = state.config;
            const newEnabled = newConfig.voiceTypingEnabled || false;
            const newShortcut = newConfig.voiceTypingShortcut || 'Alt+V';
            const newModelPath = newConfig.streamingModelPath || '';
            const newLanguage = newConfig.language || 'auto';
            const newEnableITN = newConfig.enableITN ?? true;

            const enabledChanged = newEnabled !== this.lastEnabled;
            const shortcutChanged = newShortcut !== this.lastShortcut;
            const configChanged =
                newModelPath !== this.lastModelPath ||
                newLanguage !== this.lastLanguage ||
                newEnableITN !== this.lastEnableITN;

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
            return;
        }

        try {
            logger.info('[VoiceTypingService] Pre-warming transcription model and microphone...');
            this.configureTranscriptionService();
            await this.transcriptionService.prepare();

            const lastPosition = this.sessionMachine.getLastPosition() ?? [0, 0];
            await this.overlayPresenter.prepare(lastPosition);
            await this.ensureMicrophoneStarted();

            logger.info('[VoiceTypingService] Model and mic pre-warmed and ready.');
        } catch (error) {
            logger.error('[VoiceTypingService] Failed to pre-warm:', error);
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
                return;
            }

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

                if (event.state === 'Released') {
                    if (this.sessionMachine.isActive()) {
                        void this.stopListening();
                    } else {
                        void this.startListening();
                    }
                }
            });

            this.isShortcutRegistered = true;
            this.currentShortcut = normalizedShortcut;
            logger.info('[VoiceTypingService] Successfully registered voice typing shortcut', {
                shortcut: normalizedShortcut,
            });
        } catch (error) {
            logger.error('[VoiceTypingService] Failed to update voice typing shortcut:', error);
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

    private async getOverlayPosition(): Promise<[number, number]> {
        const marginCompensation = 4;

        try {
            const cursorPosition = await invoke<[number, number] | null>(
                'get_text_cursor_position'
            );
            if (cursorPosition) {
                return [
                    cursorPosition[0] - marginCompensation,
                    cursorPosition[1] + CURSOR_POSITION_OFFSET - marginCompensation,
                ];
            }
        } catch (error) {
            logger.debug(
                '[VoiceTypingService] Failed to get text cursor position, falling back to mouse.',
                error
            );
        }

        const lastPosition = this.sessionMachine.getLastPosition();
        if (lastPosition) {
            return lastPosition;
        }

        const [x, y] = await invoke<[number, number]>('get_mouse_position');
        return [x - marginCompensation, y + MOUSE_POSITION_OFFSET - marginCompensation];
    }

    resetForTest() {
        this.initialized = false;
        this.isShortcutRegistered = false;
        this.currentShortcut = null;
        this.captureStarted = false;
        this.lastEnabled = false;
        this.lastShortcut = '';
        this.lastModelPath = '';
        this.lastLanguage = '';
        this.lastEnableITN = true;
        this.overlayPresenter.resetForTest();
        this.sessionMachine.resetForTest();
    }
}

export const voiceTypingService = new VoiceTypingService();
