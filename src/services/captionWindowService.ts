import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emit } from '@tauri-apps/api/event';
import { TranscriptSegment } from '../types/transcript';

const CAPTION_WINDOW_LABEL = 'caption';
const CAPTION_EVENT_SEGMENTS = 'caption:segments';
const CAPTION_EVENT_CLOSE = 'caption:close';

class CaptionWindowService {
    private windowInstance: WebviewWindow | null = null;

    /**
     * Opens the always-on-top caption window.
     * If it already exists, it focuses it.
     */
    async open(options?: { alwaysOnTop?: boolean, lockWindow?: boolean }) {
        // Check if window already exists
        const existingWindow = await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
        if (existingWindow) {
            await existingWindow.setFocus();
            this.windowInstance = existingWindow;
            // Apply settings if provided
            if (options?.alwaysOnTop !== undefined) {
                await this.setAlwaysOnTop(options.alwaysOnTop);
            }
            if (options?.lockWindow !== undefined) {
                await this.setClickThrough(options.lockWindow);
            }
            return;
        }

        // specific creation options for caption window
        this.windowInstance = new WebviewWindow(CAPTION_WINDOW_LABEL, {
            url: '/index.html?window=caption',
            title: 'Sona Live Caption',
            alwaysOnTop: options?.alwaysOnTop ?? true,
            decorations: false,
            transparent: true,
            skipTaskbar: true,
            width: 800,
            height: 120,
            minWidth: 200,
            minHeight: 32,
            center: false,
            focus: false,
            resizable: true,
            maximizable: false,
            minimizable: false,
            shadow: false,
        });

        // Wait for window to be created
        this.windowInstance.once('tauri://created', async () => {
            console.log('Caption window created successfully');
            // Position at bottom center (manual calculation or let OS handle initial place)
            // For now we let it float, user can drag it.

            // Apply click-through if requested (cannot be set in constructor)
            if (options?.lockWindow) {
                await this.setClickThrough(true);
            }
        });

        this.windowInstance.once('tauri://error', (e) => {
            console.error('Error creating caption window:', e);
            this.windowInstance = null;
        });
    }

    /**
     * Closes the caption window if it exists.
     */
    async close() {
        console.log('[CaptionWindowService] Requested to close caption window');

        // Robust close: Emit event first to ensure internal listener triggers close
        try {
            console.log('[CaptionWindowService] Emitting close event to window');
            await emit(CAPTION_EVENT_CLOSE);
        } catch (e) {
            console.error('[CaptionWindowService] Error emitting close event:', e);
        }

        // Ensure instance is closed if we have it
        if (this.windowInstance) {
            try {
                console.log('[CaptionWindowService] Closing cached window instance');
                await this.windowInstance.close();
            } catch (e) {
                console.error('[CaptionWindowService] Error closing cached window instance:', e);
            }
            this.windowInstance = null;
        }

        // Always try to find by label and close, just in case our instance reference was stale or lost
        try {
            const w = await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
            if (w) {
                console.log('[CaptionWindowService] Found window by label, closing it');
                await w.close();
            } else {
                console.log('[CaptionWindowService] No window found by label to close');
            }
        } catch (e) {
            // Ignore error if window not found or already closed (common in Tauri)
            // But log if it's something else
            const msg = e instanceof Error ? e.message : String(e);
            if (!msg.includes('window not found')) {
                console.error('[CaptionWindowService] Error finding/closing caption window by label:', e);
            }
        }
    }

    /**
     * Checks if the caption window is currently open.
     */
    async isOpen(): Promise<boolean> {
        const w = await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
        return !!w;
    }

    /**
     * Sends the latest segments to the caption window.
     * @param segments The list of segments to display (usually the last N)
     */
    async sendSegments(segments: TranscriptSegment[]) {
        // We broadcast the event to all windows, the caption window listens for it.
        // This is more reliable than targeting a specific window instance that might be stale.
        await emit(CAPTION_EVENT_SEGMENTS, segments);
    }

    /**
     * Sets whether the caption window should be always on top.
     * @param enabled True to enable always on top.
     */
    async setAlwaysOnTop(enabled: boolean) {
        const win = this.windowInstance || await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
        if (win) {
            await win.setAlwaysOnTop(enabled);
        }
    }

    /**
     * Sets whether the caption window should be click-through (ignore mouse events).
     * @param enabled True to enable click-through (lock window).
     */
    async setClickThrough(enabled: boolean) {
        const win = this.windowInstance || await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
        if (win) {
            await win.setIgnoreCursorEvents(enabled);
        }
    }
}

export const captionWindowService = new CaptionWindowService();
