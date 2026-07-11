import { AuxWindowController, AuxWindowControllerOptions, AuxWindowDisplayState } from './auxWindowController';
import { TauriEvent } from './tauri/events';
import { WebviewWindow } from './tauri/platform/windows';

export const VOICE_TYPING_WINDOW_LABEL = 'voice-typing';
export const VOICE_TYPING_EVENT_TEXT = TauriEvent.auxWindow.voiceTypingText;
export const VOICE_TYPING_WINDOW_WIDTH = 400;
export const VOICE_TYPING_WINDOW_INITIAL_HEIGHT = 80;
const VOICE_TYPING_WINDOW_SIZE = {
    width: VOICE_TYPING_WINDOW_WIDTH,
    height: VOICE_TYPING_WINDOW_INITIAL_HEIGHT,
};

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

export interface VoiceTypingWindowServicePorts {
    createAuxWindowController: (options: AuxWindowControllerOptions) => AuxWindowController<VoiceTypingOverlayPayload>;
    createWebviewWindow: (label: string, options: ConstructorParameters<typeof WebviewWindow>[1]) => WebviewWindow;
}

export class VoiceTypingWindowService {
    private controller: AuxWindowController<VoiceTypingOverlayPayload>;

    constructor(private readonly ports: VoiceTypingWindowServicePorts) {
        this.controller = this.ports.createAuxWindowController({
            label: VOICE_TYPING_WINDOW_LABEL,
            eventName: VOICE_TYPING_EVENT_TEXT,
            createWindow: (displayState: AuxWindowDisplayState, creationState: { visible: boolean }) =>
                this.ports.createWebviewWindow(VOICE_TYPING_WINDOW_LABEL, {
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
                    visible: creationState.visible,
                }),
        });
    }

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

export function createVoiceTypingWindowService(ports: VoiceTypingWindowServicePorts): VoiceTypingWindowService {
    return new VoiceTypingWindowService(ports);
}

export const voiceTypingWindowService = createVoiceTypingWindowService({
    createAuxWindowController: (options) => new AuxWindowController<VoiceTypingOverlayPayload>(options),
    createWebviewWindow: (label, options) => new WebviewWindow(label, options),
});
