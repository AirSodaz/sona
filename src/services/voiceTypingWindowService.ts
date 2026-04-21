import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { PhysicalPosition } from '@tauri-apps/api/dpi';
import { emit } from '@tauri-apps/api/event';
import { logger } from '../utils/logger';

const VOICE_TYPING_WINDOW_LABEL = 'voice-typing';
const DEFAULT_WINDOW_POSITION: [number, number] = [0, 0];

export const VOICE_TYPING_EVENT_TEXT = 'voice-typing:text';
export type VoiceTypingOverlayPhase = 'preparing' | 'listening' | 'segment' | 'error';

export interface VoiceTypingOverlayPayload {
    sessionId: string;
    text: string;
    phase: VoiceTypingOverlayPhase;
    segmentId?: string;
    isFinal?: boolean;
}

class VoiceTypingWindowService {
    private windowInstance: WebviewWindow | null = null;
    private lastPayload: VoiceTypingOverlayPayload | null = null;
    private windowCreationPromise: Promise<WebviewWindow | null> | null = null;

    private async createWindow(x: number, y: number): Promise<WebviewWindow | null> {
        return await new Promise((resolve) => {
            const windowInstance = new WebviewWindow(VOICE_TYPING_WINDOW_LABEL, {
                url: `/index.html?window=voice-typing`,
                title: 'Sona Voice Typing',
                alwaysOnTop: true,
                decorations: false,
                transparent: true,
                skipTaskbar: true,
                width: 400,
                height: 60,
                x,
                y,
                center: false,
                focus: false,
                resizable: false,
                maximizable: false,
                minimizable: false,
                shadow: false,
                visible: false,
            });

            this.windowInstance = windowInstance;

            windowInstance.once('tauri://created', async () => {
                logger.info('[VoiceTypingWindowService] Voice typing window created successfully');
                resolve(windowInstance);
            });

            windowInstance.once('tauri://destroyed', () => {
                this.windowInstance = null;
            });

            windowInstance.once('tauri://error', (e) => {
                logger.error('[VoiceTypingWindowService] Error creating voice typing window:', e);
                this.windowInstance = null;
                resolve(null);
            });
        });
    }

    private async ensureWindow(position: [number, number] = DEFAULT_WINDOW_POSITION) {
        if (this.windowInstance) {
            return this.windowInstance;
        }

        const existingWindow = await WebviewWindow.getByLabel(VOICE_TYPING_WINDOW_LABEL);
        if (existingWindow) {
            this.windowInstance = existingWindow;
            return existingWindow;
        }

        if (!this.windowCreationPromise) {
            this.windowCreationPromise = this.createWindow(position[0], position[1]);
        }

        const createdWindow = await this.windowCreationPromise;
        this.windowCreationPromise = null;
        return createdWindow;
    }

    async prepare(position: [number, number] = DEFAULT_WINDOW_POSITION) {
        await this.ensureWindow(position);
    }

    async open(x: number, y: number) {
        const windowInstance = await this.ensureWindow([x, y]);
        if (!windowInstance) {
            return;
        }

        await windowInstance.setPosition(new PhysicalPosition(x, y));
        await windowInstance.show();
    }

    async close() {
        const windowInstance = await this.ensureWindow();
        if (!windowInstance) {
            return;
        }

        try {
            await windowInstance.hide();
        } catch (e) {
            logger.error('[VoiceTypingWindowService] Error hiding window:', e);
        }
    }

    async sendState(payload: VoiceTypingOverlayPayload) {
        this.lastPayload = payload;
        await this.ensureWindow();

        try {
            await emit<VoiceTypingOverlayPayload>(VOICE_TYPING_EVENT_TEXT, payload);
        } catch (e) {
            logger.error('[VoiceTypingWindowService] Error emitting overlay state event:', e);
        }
    }

    getLastPayload() {
        return this.lastPayload;
    }
}

export const voiceTypingWindowService = new VoiceTypingWindowService();
