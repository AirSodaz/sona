import { describe, it, expect, vi, beforeEach, Mock } from 'vitest';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emit } from '@tauri-apps/api/event';

// Mock emit
vi.mock('@tauri-apps/api/event', () => ({
    emit: vi.fn(),
}));

// Mock DPI
vi.mock('@tauri-apps/api/dpi', () => ({
    PhysicalSize: vi.fn(),
}));

// Mock WebviewWindow
vi.mock('@tauri-apps/api/webviewWindow', () => {
    const MockWebviewWindow = vi.fn();
    (MockWebviewWindow as any).getByLabel = vi.fn();
    return { WebviewWindow: MockWebviewWindow };
});

import { captionWindowService } from '../captionWindowService';

describe('CaptionWindowService', () => {
    let mockWindowInstance: any;

    beforeEach(() => {
        vi.clearAllMocks();

        mockWindowInstance = {
            once: vi.fn(),
            close: vi.fn(),
            setFocus: vi.fn(),
            setAlwaysOnTop: vi.fn(),
            setIgnoreCursorEvents: vi.fn(),
            scaleFactor: vi.fn().mockResolvedValue(1),
            innerSize: vi.fn().mockResolvedValue({ width: 800, height: 120 }),
            setSize: vi.fn(),
        };

        // When constructor is called, return mock instance
        (WebviewWindow as unknown as Mock).mockImplementation(function() { return mockWindowInstance; });

        // Default behavior: window does not exist
        (WebviewWindow.getByLabel as any).mockResolvedValue(null);
    });

    it('opens the window with correct default properties', async () => {
        await captionWindowService.open();

        expect(WebviewWindow).toHaveBeenCalledWith('caption', expect.objectContaining({
            url: '/index.html?window=caption&width=800&fontSize=24',
            resizable: true,
            maximizable: false,
            minimizable: false,
            transparent: true,
            decorations: false,
            alwaysOnTop: true
        }));
    });

    it('opens the window with custom style properties', async () => {
        await captionWindowService.open({ width: 1000, fontSize: 32 });

        expect(WebviewWindow).toHaveBeenCalledWith('caption', expect.objectContaining({
            url: '/index.html?window=caption&width=1000&fontSize=32',
            width: 1000,
        }));
    });

    it('reuses existing window and updates style', async () => {
        (WebviewWindow.getByLabel as any).mockResolvedValue(mockWindowInstance);

        await captionWindowService.open({ width: 1200, fontSize: 40 });

        // Constructor not called
        expect(WebviewWindow).toHaveBeenCalledTimes(0);

        // Focus called
        expect(mockWindowInstance.setFocus).toHaveBeenCalled();

        // Should update style
        expect(emit).toHaveBeenCalledWith('caption:style', { width: 1200, fontSize: 40 });

        // Should resize window
        expect(mockWindowInstance.setSize).toHaveBeenCalled();
    });
});
