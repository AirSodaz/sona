import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emit } from '@tauri-apps/api/event';
import { TranscriptSegment } from '../types/transcript';

const CAPTION_WINDOW_LABEL = 'caption';
const CAPTION_EVENT_SEGMENTS = 'caption:segments';

class CaptionWindowService {
    private windowInstance: WebviewWindow | null = null;

    /**
     * Opens the always-on-top caption window.
     * If it already exists, it focuses it.
     */
    async open() {
        // Check if window already exists
        const existingWindow = await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
        if (existingWindow) {
            await existingWindow.setFocus();
            this.windowInstance = existingWindow;
            return;
        }

        // specific creation options for caption window
        this.windowInstance = new WebviewWindow(CAPTION_WINDOW_LABEL, {
            url: '/index.html?window=caption',
            title: 'Sona Live Caption',
            alwaysOnTop: true,
            decorations: false,
            transparent: true,
            skipTaskbar: true,
            width: 800,
            height: 200,
            center: false,
            focus: false,
            resizable: true,
            shadow: false,
        });

        // Wait for window to be created
        this.windowInstance.once('tauri://created', async () => {
            console.log('Caption window created successfully');
            // Position at bottom center (manual calculation or let OS handle initial place)
            // For now we let it float, user can drag it.
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
        const w = await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
        if (w) {
            await w.close();
            this.windowInstance = null;
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
}

export const captionWindowService = new CaptionWindowService();
