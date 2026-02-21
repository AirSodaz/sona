import { describe, it, expect, vi, beforeEach } from 'vitest';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

// Define mocks before importing the service
vi.mock('@tauri-apps/api/webviewWindow', () => {
    const MockWebviewWindow = vi.fn();
    MockWebviewWindow.prototype.once = vi.fn();
    MockWebviewWindow.prototype.close = vi.fn();
    MockWebviewWindow.prototype.setFocus = vi.fn();
    MockWebviewWindow.prototype.setAlwaysOnTop = vi.fn();
    MockWebviewWindow.prototype.setIgnoreCursorEvents = vi.fn();
    (MockWebviewWindow as any).getByLabel = vi.fn();
    return { WebviewWindow: MockWebviewWindow };
});

vi.mock('@tauri-apps/api/event', () => ({
    emit: vi.fn(),
}));

// Import service after mocks
import { captionWindowService } from '../captionWindowService';

describe('CaptionWindowService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        // Default behavior: window does not exist
        (WebviewWindow.getByLabel as any).mockResolvedValue(null);
    });

    it('opens the window with correct properties', async () => {
        await captionWindowService.open();

        expect(WebviewWindow).toHaveBeenCalledWith('caption', expect.objectContaining({
            resizable: true,
            maximizable: false,
            minimizable: false,
            transparent: true,
            decorations: false,
            alwaysOnTop: true
        }));
    });

    it('reuses existing window if open', async () => {
        const mockExistingWindow = {
            setFocus: vi.fn(),
        };
        (WebviewWindow.getByLabel as any).mockResolvedValue(mockExistingWindow);

        // We need to ensure we don't use the cached instance from previous tests
        // Since captionWindowService is a singleton, verifying state isolation is tricky without resetting it.
        // However, `open` calls `getByLabel` first. If `getByLabel` returns something, it uses it.

        await captionWindowService.open();

        // The constructor should NOT be called again if it finds an existing one
        // Wait, if the previous test ran, `captionWindowService` might have `this.windowInstance` set?
        // Let's check the code:
        /*
        async open() {
            const existingWindow = await WebviewWindow.getByLabel(CAPTION_WINDOW_LABEL);
            if (existingWindow) { ... return; }
            this.windowInstance = new WebviewWindow(...)
        */
        // Even if `this.windowInstance` is set, `open` checks `getByLabel`.
        // So checking `WebviewWindow` constructor calls is safe.

        expect(WebviewWindow).toHaveBeenCalledTimes(0);
        expect(mockExistingWindow.setFocus).toHaveBeenCalled();
    });
});
