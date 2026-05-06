import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { TranscriptSegment } from '../types/transcript';
import { AuxWindowController } from './auxWindowController';
import { TauriEvent } from './tauri/events';
import { logger } from '../utils/logger';

export const CAPTION_WINDOW_LABEL = 'caption';
export const CAPTION_EVENT_STATE = TauriEvent.auxWindow.captionState;
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

function areCaptionStylesEqual(
    left: CaptionWindowStyle,
    right: CaptionWindowStyle
): boolean {
    return left.width === right.width
        && left.fontSize === right.fontSize
        && left.color === right.color
        && left.backgroundOpacity === right.backgroundOpacity;
}

function isWindowNotFoundError(error: unknown): boolean {
    if (error instanceof Error) {
        return error.message.toLowerCase().includes('window not found');
    }

    return typeof error === 'string'
        && error.toLowerCase().includes('window not found');
}

async function applyCaptionWindowFlag(
    action: string,
    operation: () => Promise<void>
): Promise<void> {
    try {
        await operation();
    } catch (error) {
        const logPayload = { action, error };
        if (isWindowNotFoundError(error)) {
            await logger.debug('[CaptionWindowService] Caption window flag skipped', logPayload);
            return;
        }

        await logger.warn('[CaptionWindowService] Failed to apply caption window flag', logPayload);
    }
}

class CaptionWindowService {
    private controller = new AuxWindowController<CaptionWindowState>({
        label: CAPTION_WINDOW_LABEL,
        eventName: CAPTION_EVENT_STATE,
        createWindow: (displayState, creationState) =>
            new WebviewWindow(CAPTION_WINDOW_LABEL, {
                url: '/index.html?window=caption',
                title: 'Sona Live Caption',
                alwaysOnTop: displayState.alwaysOnTop ?? true,
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
                visible: creationState.visible,
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
        logger.info('[CaptionWindowService] Opening caption window', {
            width: options?.width ?? this.state.style.width,
            fontSize: options?.fontSize ?? this.state.style.fontSize,
            color: options?.color ?? this.state.style.color,
            backgroundOpacity:
                options?.backgroundOpacity ?? this.state.style.backgroundOpacity,
            alwaysOnTop: options?.alwaysOnTop ?? true,
            lockWindow: options?.lockWindow ?? false,
        });

        const nextStyle: CaptionWindowStyle = {
            width: options?.width ?? this.state.style.width,
            fontSize: options?.fontSize ?? this.state.style.fontSize,
            color: options?.color ?? this.state.style.color,
            backgroundOpacity:
                options?.backgroundOpacity ?? this.state.style.backgroundOpacity,
        };

        await this.commitState({ style: nextStyle });

        const requestedAlwaysOnTop = options?.alwaysOnTop;
        const requestedLockWindow = options?.lockWindow;
        const hadExistingWindow = Boolean(await this.controller.getWindow());
        const windowInstance = await this.controller.open({
            size: { width: nextStyle.width, height: CAPTION_INITIAL_HEIGHT },
            focus: true,
            alwaysOnTop: requestedAlwaysOnTop ?? true,
        });

        if (!windowInstance) {
            logger.error('[CaptionWindowService] Failed to obtain caption window handle');
            return;
        }

        if (requestedAlwaysOnTop !== undefined && hadExistingWindow) {
            await applyCaptionWindowFlag(
                'alwaysOnTop',
                () => windowInstance.setAlwaysOnTop(requestedAlwaysOnTop)
            );
        }

        if (requestedLockWindow !== undefined) {
            await applyCaptionWindowFlag(
                'lockWindow',
                () => windowInstance.setIgnoreCursorEvents(requestedLockWindow)
            );
        }

        logger.info('[CaptionWindowService] Caption window open request completed');
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
        logger.info('[CaptionWindowService] Sending caption segments', {
            segmentCount: latestSegments.length,
            latestSegmentId: latestSegments[0]?.id ?? null,
            latestSegmentFinal: latestSegments[0]?.isFinal ?? null,
            textLength: latestSegments[0]?.text.length ?? 0,
        });
        await this.commitState({ segments: latestSegments });
    }

    async setAlwaysOnTop(enabled: boolean) {
        const windowInstance = await this.controller.getWindow();
        if (windowInstance) {
            await applyCaptionWindowFlag(
                'alwaysOnTop',
                () => windowInstance.setAlwaysOnTop(enabled)
            );
        }
    }

    async setClickThrough(enabled: boolean) {
        const windowInstance = await this.controller.getWindow();
        if (windowInstance) {
            await applyCaptionWindowFlag(
                'clickThrough',
                () => windowInstance.setIgnoreCursorEvents(enabled)
            );
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

        if (areCaptionStylesEqual(nextStyle, this.state.style)) {
            return;
        }

        await this.commitState({ style: nextStyle });
    }

    async getSnapshot() {
        return await this.controller.getState();
    }
}

export const captionWindowService = new CaptionWindowService();
