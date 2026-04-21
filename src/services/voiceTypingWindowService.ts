import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { AuxWindowController } from './auxWindowController';

export const VOICE_TYPING_WINDOW_LABEL = 'voice-typing';
export const VOICE_TYPING_EVENT_TEXT = 'voice-typing:text';
const VOICE_TYPING_WINDOW_SIZE = { width: 400, height: 60 };

export type VoiceTypingOverlayPhase = 'preparing' | 'listening' | 'segment' | 'error';

export interface VoiceTypingOverlayPayload {
    sessionId: string;
    text: string;
    phase: VoiceTypingOverlayPhase;
    revision: number;
    segmentId?: string;
    isFinal?: boolean;
}

export const DEFAULT_VOICE_TYPING_OVERLAY_STATE: VoiceTypingOverlayPayload = {
    sessionId: 'bootstrap',
    text: '',
    phase: 'listening',
    revision: 0,
};

class VoiceTypingWindowService {
    private controller = new AuxWindowController<VoiceTypingOverlayPayload>({
        label: VOICE_TYPING_WINDOW_LABEL,
        eventName: VOICE_TYPING_EVENT_TEXT,
        createWindow: (displayState) => new WebviewWindow(VOICE_TYPING_WINDOW_LABEL, {
            url: '/index.html?window=voice-typing',
            title: 'Sona Voice Typing',
            alwaysOnTop: true,
            decorations: false,
            transparent: true,
            skipTaskbar: true,
            width: displayState.size?.width ?? VOICE_TYPING_WINDOW_SIZE.width,
            height: displayState.size?.height ?? VOICE_TYPING_WINDOW_SIZE.height,
            x: displayState.position?.[0] ?? 0,
            y: displayState.position?.[1] ?? 0,
            center: false,
            focus: false,
            resizable: false,
            maximizable: false,
            minimizable: false,
            shadow: false,
            visible: false,
        }),
    });

    async prepare(position: [number, number]) {
        await this.controller.prepare({
            position,
            size: VOICE_TYPING_WINDOW_SIZE,
        });
    }

    async open(x: number, y: number) {
        await this.controller.open({
            position: [x, y],
            size: VOICE_TYPING_WINDOW_SIZE,
        });
    }

    async close() {
        await this.controller.hide();
    }

    async sendState(payload: VoiceTypingOverlayPayload) {
        await this.controller.commitState(payload);
    }

    async getSnapshot() {
        return await this.controller.getState();
    }

    async clearState() {
        await this.controller.clearState();
    }

    getLastPayload() {
        return this.controller.getLastState();
    }
}

export const voiceTypingWindowService = new VoiceTypingWindowService();
