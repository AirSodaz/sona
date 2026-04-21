import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { TranscriptSegment } from '../types/transcript';
import { AuxWindowController } from './auxWindowController';
import { logger } from '../utils/logger';

export const CAPTION_WINDOW_LABEL = 'caption';
export const CAPTION_EVENT_STATE = 'caption:state';
const CAPTION_INITIAL_HEIGHT = 120;

export interface CaptionWindowStyle {
    width: number;
    fontSize: number;
    color: string;
    backgroundOpacity: number;
}

export interface CaptionWindowState {
    revision: number;
    segments: TranscriptSegment[];
    style: CaptionWindowStyle;
}

const DEFAULT_CAPTION_STYLE: CaptionWindowStyle = {
    width: 800,
    fontSize: 24,
    color: '#ffffff',
    backgroundOpacity: 0.6,
};

export const DEFAULT_CAPTION_WINDOW_STATE: CaptionWindowState = {
    revision: 0,
    segments: [],
    style: DEFAULT_CAPTION_STYLE,
};

class CaptionWindowService {
    private controller = new AuxWindowController<CaptionWindowState>({
        label: CAPTION_WINDOW_LABEL,
        eventName: CAPTION_EVENT_STATE,
        createWindow: (displayState) => new WebviewWindow(CAPTION_WINDOW_LABEL, {
            url: '/index.html?window=caption',
            title: 'Sona Live Caption',
            alwaysOnTop: true,
            decorations: false,
            transparent: true,
            skipTaskbar: true,
            width: displayState.size?.width ?? DEFAULT_CAPTION_STYLE.width,
            height: displayState.size?.height ?? CAPTION_INITIAL_HEIGHT,
            minWidth: 200,
            minHeight: 32,
            center: false,
            focus: false,
            resizable: true,
            maximizable: false,
            minimizable: false,
            shadow: false,
            visible: false,
        }),
    });

    private state: CaptionWindowState = DEFAULT_CAPTION_WINDOW_STATE;

    private buildState(partial: Partial<Omit<CaptionWindowState, 'revision'>>) {
        this.state = {
            revision: this.state.revision + 1,
            segments: partial.segments ?? this.state.segments,
            style: partial.style ?? this.state.style,
        };

        return this.state;
    }

    private async commitState(partial: Partial<Omit<CaptionWindowState, 'revision'>>) {
        const nextState = this.buildState(partial);
        await this.controller.commitState(nextState);
        return nextState;
    }

    async open(options?: {
        alwaysOnTop?: boolean;
        lockWindow?: boolean;
        width?: number;
        fontSize?: number;
        color?: string;
        backgroundOpacity?: number;
    }) {
        const nextStyle: CaptionWindowStyle = {
            width: options?.width ?? this.state.style.width,
            fontSize: options?.fontSize ?? this.state.style.fontSize,
            color: options?.color ?? this.state.style.color,
            backgroundOpacity:
                options?.backgroundOpacity ?? this.state.style.backgroundOpacity,
        };

        await this.commitState({ style: nextStyle });

        const windowInstance = await this.controller.open({
            size: { width: nextStyle.width, height: CAPTION_INITIAL_HEIGHT },
            focus: true,
        });

        if (!windowInstance) {
            return;
        }

        if (options?.alwaysOnTop !== undefined) {
            await windowInstance.setAlwaysOnTop(options.alwaysOnTop);
        }

        if (options?.lockWindow !== undefined) {
            await windowInstance.setIgnoreCursorEvents(options.lockWindow);
        }
    }

    async close() {
        logger.info('[CaptionWindowService] Requested to close caption window');
        this.state = {
            ...this.state,
            revision: this.state.revision + 1,
            segments: [],
        };
        await this.controller.clearState();
        await this.controller.close();
    }

    async isOpen(): Promise<boolean> {
        const windowInstance = await this.controller.getWindow();
        return !!windowInstance;
    }

    async sendSegments(segments: TranscriptSegment[]) {
        const latestSegments = segments.length > 0 ? [segments[segments.length - 1]] : [];
        await this.commitState({ segments: latestSegments });
    }

    async setAlwaysOnTop(enabled: boolean) {
        const windowInstance = await this.controller.getWindow();
        if (windowInstance) {
            await windowInstance.setAlwaysOnTop(enabled);
        }
    }

    async setClickThrough(enabled: boolean) {
        const windowInstance = await this.controller.getWindow();
        if (windowInstance) {
            await windowInstance.setIgnoreCursorEvents(enabled);
        }
    }

    async updateStyle(style: {
        width?: number;
        fontSize?: number;
        color?: string;
        backgroundOpacity?: number;
    }) {
        const nextStyle: CaptionWindowStyle = {
            width: style.width ?? this.state.style.width,
            fontSize: style.fontSize ?? this.state.style.fontSize,
            color: style.color ?? this.state.style.color,
            backgroundOpacity:
                style.backgroundOpacity ?? this.state.style.backgroundOpacity,
        };

        await this.commitState({ style: nextStyle });

        if (!style.width) {
            return;
        }

        const windowInstance = await this.controller.getWindow();
        if (!windowInstance) {
            return;
        }

        try {
            const { PhysicalSize } = await import('@tauri-apps/api/dpi');
            const factor = await windowInstance.scaleFactor();
            const size = await windowInstance.innerSize();
            const targetWidth = Math.ceil(style.width * factor);
            await windowInstance.setSize(new PhysicalSize(targetWidth, size.height));
        } catch (error) {
            logger.error('[CaptionWindowService] Failed to resize window:', error);
        }
    }

    async getSnapshot() {
        return await this.controller.getState();
    }
}

export const captionWindowService = new CaptionWindowService();
