import { register, unregister, isRegistered } from '@tauri-apps/plugin-global-shortcut';
import { invoke } from '@tauri-apps/api/core';
import { TranscriptionService } from './transcriptionService';
import { voiceTypingWindowService } from './voiceTypingWindowService';
import { useConfigStore } from '../stores/configStore';
import { logger } from '../utils/logger';

class VoiceTypingService {
    private isListening = false;
    private isShortcutRegistered = false;
    private transcriptionService: TranscriptionService;
    private currentShortcut: string | null = null;

    private initialized = false;
    private lastEnabled = false;
    private lastShortcut = '';

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

        logger.info(`[VoiceTypingService] Initial config - Enabled: ${this.lastEnabled}, Shortcut: ${this.lastShortcut}`);

        // Listen to config changes to update shortcuts
        useConfigStore.subscribe((state) => {
            const newConfig = state.config;
            const newEnabled = newConfig.voiceTypingEnabled || false;
            const newShortcut = newConfig.voiceTypingShortcut || 'Alt+V';

            if (newEnabled !== this.lastEnabled || newShortcut !== this.lastShortcut) {
                logger.info(`[VoiceTypingService] Config changed. New Enabled: ${newEnabled}, New Shortcut: ${newShortcut}`);
                this.lastEnabled = newEnabled;
                this.lastShortcut = newShortcut;
                this.updateShortcutRegistration(newEnabled, newShortcut);
            }
        });

        // Initial setup
        this.updateShortcutRegistration(this.lastEnabled, this.lastShortcut);
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
        
        try {
            // Get mouse position and open window
            const [x, y] = await invoke<[number, number]>('get_mouse_position');
            // Offset by 20px below cursor
            await voiceTypingWindowService.open(x, y + 20);
            await voiceTypingWindowService.sendText('正在准备录音...');

            // Ensure model is ready
            const config = useConfigStore.getState().config;
            this.transcriptionService.setModelPath(config.streamingModelPath);
            this.transcriptionService.setLanguage(config.language);
            this.transcriptionService.setEnableITN(config.enableITN ?? true);
            await this.transcriptionService.prepare();

            // Start audio capture
            await invoke('start_microphone_capture', {
                deviceName: config.microphoneId === 'default' ? null : config.microphoneId,
                instanceId: 'voice-typing'
            });

            // Start transcription
            let currentText = '';
            await this.transcriptionService.start(
                async (segment) => {
                    if (segment.isFinal) {
                        const finalSegmentText = segment.text;
                        if (finalSegmentText.trim()) {
                            await invoke('inject_text', { text: finalSegmentText });
                        }
                        currentText = ''; // Clear for next segment
                        await voiceTypingWindowService.sendText('正在聆听...');
                    } else {
                        currentText = segment.text;
                        await voiceTypingWindowService.sendText(currentText);
                    }
                },
                (error) => {
                    logger.error('Voice typing transcription error:', error);
                    voiceTypingWindowService.sendText('语音识别出错: ' + error);
                    this.stopListening();
                }
            );

        } catch (e) {
            logger.error('Failed to start voice typing:', e);
            this.isListening = false;
            await voiceTypingWindowService.close();
        }
    }

    private async stopListening() {
        if (!this.isListening) return;
        this.isListening = false;

        try {
            await invoke('stop_microphone_capture', { instanceId: 'voice-typing' });
            await this.transcriptionService.stop();
            await voiceTypingWindowService.close();
        } catch (e) {
            logger.error('Failed to stop voice typing:', e);
        }
    }
}

export const voiceTypingService = new VoiceTypingService();
