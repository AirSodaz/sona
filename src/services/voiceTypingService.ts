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

    constructor() {
        this.transcriptionService = new TranscriptionService('voice-typing');
        
        // Listen to config changes to update shortcuts
        useConfigStore.subscribe((state, prev) => {
            const newConfig = state.config;
            const prevConfig = prev.config;
            if (newConfig.voiceTypingEnabled !== prevConfig.voiceTypingEnabled ||
                newConfig.voiceTypingShortcut !== prevConfig.voiceTypingShortcut) {
                this.updateShortcutRegistration(newConfig.voiceTypingEnabled || false, newConfig.voiceTypingShortcut || 'Alt+V');
            }
        });

        // Initial setup
        const config = useConfigStore.getState().config;
        this.updateShortcutRegistration(config.voiceTypingEnabled || false, config.voiceTypingShortcut || 'Alt+V');
    }

    private async updateShortcutRegistration(enabled: boolean, shortcut: string) {
        try {
            if (this.isShortcutRegistered && this.currentShortcut) {
                const registered = await isRegistered(this.currentShortcut);
                if (registered) {
                    await unregister(this.currentShortcut);
                }
                this.isShortcutRegistered = false;
            }

            if (enabled && shortcut) {
                await register(shortcut, (event) => {
                    const mode = useConfigStore.getState().config.voiceTypingMode || 'hold';
                    
                    if (mode === 'hold') {
                        if (event.state === 'Pressed' && !this.isListening) {
                            this.startListening();
                        } else if (event.state === 'Released' && this.isListening) {
                            this.stopListening();
                        }
                    } else if (mode === 'toggle') {
                        if (event.state === 'Released') {
                            if (this.isListening) {
                                this.stopListening();
                            } else {
                                this.startListening();
                            }
                        }
                    }
                });
                this.isShortcutRegistered = true;
                this.currentShortcut = shortcut;
                logger.info(`Voice typing shortcut registered: ${shortcut}`);
            }
        } catch (e) {
            logger.error('Failed to update voice typing shortcut:', e);
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
            this.transcriptionService.setModelPath(config.offlineModelPath);
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
