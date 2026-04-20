import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { PhysicalPosition } from '@tauri-apps/api/dpi';
import { emit } from '@tauri-apps/api/event';
import { logger } from '../utils/logger';

const VOICE_TYPING_WINDOW_LABEL = 'voice-typing';
export const VOICE_TYPING_EVENT_TEXT = 'voice-typing:text';

class VoiceTypingWindowService {
    private windowInstance: WebviewWindow | null = null;

    async open(x: number, y: number) {
        const existingWindow = await WebviewWindow.getByLabel(VOICE_TYPING_WINDOW_LABEL);
        if (existingWindow) {
            await existingWindow.setPosition(new PhysicalPosition(x, y));
            await existingWindow.show();
            return;
        }

        this.windowInstance = new WebviewWindow(VOICE_TYPING_WINDOW_LABEL, {
            url: `/index.html?window=voice-typing`,
            title: 'Sona Voice Typing',
            alwaysOnTop: true,
            decorations: false,
            transparent: true,
            skipTaskbar: true,
            width: 400,
            height: 60,
            x: x,
            y: y,
            center: false,
            focus: false,
            resizable: false,
            maximizable: false,
            minimizable: false,
            shadow: false,
        });

        this.windowInstance.once('tauri://created', async () => {
            logger.info('Voice typing window created successfully');
        });

        this.windowInstance.once('tauri://error', (e) => {
            logger.error('Error creating voice typing window:', e);
            this.windowInstance = null;
        });
    }

    async close() {
        if (!this.windowInstance) {
            const existingWindow = await WebviewWindow.getByLabel(VOICE_TYPING_WINDOW_LABEL);
            if (existingWindow) {
                this.windowInstance = existingWindow;
            }
        }
        
        if (this.windowInstance) {
            try {
                await this.windowInstance.hide();
            } catch (e) {
                logger.error('[VoiceTypingWindowService] Error hiding window:', e);
            }
        }
    }

    async sendText(text: string) {
        try {
            await emit(VOICE_TYPING_EVENT_TEXT, { text });
        } catch (e) {
            logger.error('[VoiceTypingWindowService] Error emitting text event:', e);
        }
    }
}

export const voiceTypingWindowService = new VoiceTypingWindowService();
