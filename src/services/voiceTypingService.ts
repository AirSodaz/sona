import { register, unregister, isRegistered } from '@tauri-apps/plugin-global-shortcut';
import { invoke } from '@tauri-apps/api/core';
import { TranscriptionService } from './transcriptionService';
import { voiceTypingWindowService } from './voiceTypingWindowService';
import { useConfigStore } from '../stores/configStore';
import { logger } from '../utils/logger';

const CURSOR_POSITION_OFFSET = 12;
const MOUSE_POSITION_OFFSET = 20;

class VoiceTypingService {
    private isListening = false;
    private isShortcutRegistered = false;
    private transcriptionService: TranscriptionService;
    private currentShortcut: string | null = null;
    private captureStarted = false;
    private startRequestId = 0;

    private initialized = false;
    private lastEnabled = false;
    private lastShortcut = '';
    private lastModelPath = '';
    private lastLanguage = '';
    private lastEnableITN = true;

    constructor() {
        this.transcriptionService = new TranscriptionService('voice-typing');
    }

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

        logger.info(`[VoiceTypingService] Initial config - Enabled: ${this.lastEnabled}, Shortcut: ${this.lastShortcut}`);

        // Listen to config changes to update shortcuts and pre-warm model
        useConfigStore.subscribe((state) => {
            const newConfig = state.config;
            const newEnabled = newConfig.voiceTypingEnabled || false;
            const newShortcut = newConfig.voiceTypingShortcut || 'Alt+V';
            const newModelPath = newConfig.streamingModelPath || '';
            const newLanguage = newConfig.language || 'auto';
            const newEnableITN = newConfig.enableITN ?? true;

            const configChanged = newModelPath !== this.lastModelPath || 
                                newLanguage !== this.lastLanguage || 
                                newEnableITN !== this.lastEnableITN;

            if (newEnabled !== this.lastEnabled || newShortcut !== this.lastShortcut) {
                logger.info(`[VoiceTypingService] Shortcut config changed. New Enabled: ${newEnabled}, New Shortcut: ${newShortcut}`);
                this.lastEnabled = newEnabled;
                this.lastShortcut = newShortcut;
                this.updateShortcutRegistration(newEnabled, newShortcut);
            }

            if (newEnabled && (configChanged || newEnabled !== this.lastEnabled)) {
                this.lastModelPath = newModelPath;
                this.lastLanguage = newLanguage;
                this.lastEnableITN = newEnableITN;
                this.syncAndPrepare();
            }
        });

        // Initial setup
        this.updateShortcutRegistration(this.lastEnabled, this.lastShortcut);
        if (this.lastEnabled) {
            this.syncAndPrepare();
        }
    }

    private async syncAndPrepare() {
        const config = useConfigStore.getState().config;
        if (!config.voiceTypingEnabled || !config.streamingModelPath) return;

        try {
            logger.info('[VoiceTypingService] Pre-warming transcription model...');
            this.transcriptionService.setModelPath(config.streamingModelPath);
            this.transcriptionService.setLanguage(config.language);
            this.transcriptionService.setEnableITN(config.enableITN ?? true);
            await this.transcriptionService.prepare();
            logger.info('[VoiceTypingService] Model pre-warmed and ready.');
        } catch (e) {
            logger.error('[VoiceTypingService] Failed to pre-warm model:', e);
        }
    }

    private async updateShortcutRegistration(enabled: boolean, shortcut: string) {
        const normalizedShortcut = shortcut.replace(/\s+/g, '');
        logger.info(`[VoiceTypingService] updateShortcutRegistration called. Enabled: ${enabled}, Shortcut: ${shortcut}, Normalized: ${normalizedShortcut}`);
        
        try {
            if (this.isShortcutRegistered && this.currentShortcut) {
                logger.info(`[VoiceTypingService] Checking if current shortcut ${this.currentShortcut} is registered...`);
                const registered = await isRegistered(this.currentShortcut);
                if (registered) {
                    logger.info(`[VoiceTypingService] Unregistering old shortcut ${this.currentShortcut}...`);
                    await unregister(this.currentShortcut);
                }
                this.isShortcutRegistered = false;
            }

            if (enabled && normalizedShortcut) {
                logger.info(`[VoiceTypingService] Attempting to register new shortcut ${normalizedShortcut}...`);
                await register(normalizedShortcut, (event) => {
                    logger.info(`[VoiceTypingService] Shortcut Event Triggered: ${event.shortcut}, State: ${event.state}`);
                    const mode = useConfigStore.getState().config.voiceTypingMode || 'hold';
                    logger.info(`[VoiceTypingService] Current Mode: ${mode}, isListening: ${this.isListening}`);
                    
                    if (mode === 'hold') {
                        if (event.state === 'Pressed' && !this.isListening) {
                            logger.info('[VoiceTypingService] Hold Mode -> Pressed -> Starting listen');
                            this.startListening();
                        } else if (event.state === 'Released' && this.isListening) {
                            logger.info('[VoiceTypingService] Hold Mode -> Released -> Stopping listen');
                            this.stopListening();
                        }
                    } else if (mode === 'toggle') {
                        if (event.state === 'Released') {
                            if (this.isListening) {
                                logger.info('[VoiceTypingService] Toggle Mode -> Released -> Stopping listen');
                                this.stopListening();
                            } else {
                                logger.info('[VoiceTypingService] Toggle Mode -> Released -> Starting listen');
                                this.startListening();
                            }
                        }
                    }
                });
                this.isShortcutRegistered = true;
                this.currentShortcut = normalizedShortcut;
                logger.info(`[VoiceTypingService] Successfully registered voice typing shortcut: ${normalizedShortcut}`);
            } else {
                logger.info(`[VoiceTypingService] Not registering shortcut (enabled: ${enabled}, shortcut: ${normalizedShortcut})`);
            }
        } catch (e) {
            logger.error('[VoiceTypingService] Failed to update voice typing shortcut:', e);
        }
    }

    private async startListening() {
        if (this.isListening) return;
        this.isListening = true;
        const requestId = ++this.startRequestId;
        
        try {
            const [x, y] = await this.getOverlayPosition();
            await voiceTypingWindowService.open(x, y);
            await voiceTypingWindowService.sendText('正在准备录音...');

            // Ensure model is ready
            const config = useConfigStore.getState().config;
            this.transcriptionService.setModelPath(config.streamingModelPath);
            this.transcriptionService.setLanguage(config.language);
            this.transcriptionService.setEnableITN(config.enableITN ?? true);
            await this.transcriptionService.prepare();

            // Start transcription
            await this.transcriptionService.start(
                async (segment) => {
                    if (segment.isFinal) {
                        const finalSegmentText = segment.text.trim();
                        if (finalSegmentText) {
                            await voiceTypingWindowService.sendText(finalSegmentText);
                            await invoke('inject_text', { text: finalSegmentText });
                        }
                    } else {
                        const partialText = segment.text.trim();
                        if (partialText) {
                            await voiceTypingWindowService.sendText(partialText);
                        }
                    }
                },
                (error) => {
                    logger.error('Voice typing transcription error:', error);
                    voiceTypingWindowService.sendText('语音识别出错: ' + error);
                    this.stopListening();
                }
            );

            if (!this.isListening || requestId !== this.startRequestId) {
                await this.transcriptionService.softStop();
                await voiceTypingWindowService.close();
                return;
            }

            // Start audio capture after the recognizer is ready to avoid dropping the first speech chunk.
            await invoke('start_microphone_capture', {
                deviceName: config.microphoneId === 'default' ? null : config.microphoneId,
                instanceId: 'voice-typing'
            });
            this.captureStarted = true;

            if (!this.isListening || requestId !== this.startRequestId) {
                await this.stopListening();
            }

        } catch (e) {
            logger.error('Failed to start voice typing:', e);
            this.isListening = false;
            this.captureStarted = false;
            await this.transcriptionService.softStop().catch((stopError) => {
                logger.error('Failed to roll back voice typing recognizer after start failure:', stopError);
            });
            await voiceTypingWindowService.close();
        }
    }

    private async stopListening() {
        if (!this.isListening && !this.captureStarted) return;
        this.startRequestId += 1;
        this.isListening = false;
        const shouldStopCapture = this.captureStarted;
        this.captureStarted = false;

        try {
            if (shouldStopCapture) {
                await invoke('stop_microphone_capture', { instanceId: 'voice-typing' });
            }
        } catch (e) {
            logger.error('Failed to stop voice typing:', e);
        } finally {
            await this.transcriptionService.softStop().catch((stopError) => {
                logger.error('Failed to flush voice typing recognizer while stopping:', stopError);
            });
            await voiceTypingWindowService.close();
        }
    }

    private async getOverlayPosition(): Promise<[number, number]> {
        try {
            const cursorPosition = await invoke<[number, number] | null>('get_text_cursor_position');
            if (cursorPosition) {
                return [cursorPosition[0], cursorPosition[1] + CURSOR_POSITION_OFFSET];
            }
        } catch (error) {
            logger.debug('[VoiceTypingService] Failed to get text cursor position, falling back to mouse.', error);
        }

        const [x, y] = await invoke<[number, number]>('get_mouse_position');
        return [x, y + MOUSE_POSITION_OFFSET];
    }
}

export const voiceTypingService = new VoiceTypingService();
