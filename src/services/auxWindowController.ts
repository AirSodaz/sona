import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi';
import { emitTo } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { auxWindowStateService } from './auxWindowStateService';
import { logger } from '../utils/logger';

export interface AuxWindowDisplayState {
    position?: [number, number] | null;
    size?: { width: number; height: number } | null;
    focus?: boolean;
}

interface AuxWindowControllerOptions {
    label: string;
    eventName: string;
    createWindow: (displayState: AuxWindowDisplayState) => WebviewWindow;
}

export class AuxWindowController<T extends object> {
    private windowInstance: WebviewWindow | null = null;
    private windowCreationPromise: Promise<WebviewWindow | null> | null = null;
    private lastState: T | null = null;

    constructor(private readonly options: AuxWindowControllerOptions) { }

    private trackWindow(windowInstance: WebviewWindow) {
        if (this.windowInstance === windowInstance) {
            return windowInstance;
        }

        this.windowInstance = windowInstance;
        windowInstance.once('tauri://destroyed', () => {
            if (this.windowInstance === windowInstance) {
                this.windowInstance = null;
            }
        });

        return windowInstance;
    }

    private async resolveExistingWindow() {
        if (this.windowInstance) {
            return this.windowInstance;
        }

        const existingWindow = await WebviewWindow.getByLabel(this.options.label);
        if (!existingWindow) {
            return null;
        }

        return this.trackWindow(existingWindow);
    }

    private async createWindow(displayState: AuxWindowDisplayState) {
        return await new Promise<WebviewWindow | null>((resolve) => {
            const windowInstance = this.trackWindow(this.options.createWindow(displayState));

            windowInstance.once('tauri://created', () => {
                logger.info('[AuxWindowController] Window created', { label: this.options.label });
                resolve(windowInstance);
            });

            windowInstance.once('tauri://error', (error) => {
                logger.error('[AuxWindowController] Window creation failed', {
                    label: this.options.label,
                    error,
                });
                if (this.windowInstance === windowInstance) {
                    this.windowInstance = null;
                }
                resolve(null);
            });
        });
    }

    private async ensureWindow(displayState: AuxWindowDisplayState = {}) {
        const existingWindow = await this.resolveExistingWindow();
        if (existingWindow) {
            return existingWindow;
        }

        if (!this.windowCreationPromise) {
            this.windowCreationPromise = this.createWindow(displayState);
        }

        const createdWindow = await this.windowCreationPromise;
        this.windowCreationPromise = null;
        return createdWindow;
    }

    private async applyDisplayState(
        windowInstance: WebviewWindow,
        displayState: AuxWindowDisplayState,
        visible: boolean
    ) {
        if (displayState.size) {
            await windowInstance.setSize(
                new PhysicalSize(displayState.size.width, displayState.size.height)
            );
        }

        if (displayState.position) {
            await windowInstance.setPosition(
                new PhysicalPosition(displayState.position[0], displayState.position[1])
            );
        }

        if (visible) {
            await windowInstance.show();
            if (displayState.focus) {
                await windowInstance.setFocus();
            }
            return;
        }

        await windowInstance.hide();
    }

    async prepare(displayState: AuxWindowDisplayState = {}) {
        const windowInstance = await this.ensureWindow(displayState);
        if (!windowInstance) {
            return null;
        }

        await this.applyDisplayState(windowInstance, displayState, false);
        return windowInstance;
    }

    async open(displayState: AuxWindowDisplayState = {}) {
        const windowInstance = await this.ensureWindow(displayState);
        if (!windowInstance) {
            return null;
        }

        await this.applyDisplayState(windowInstance, displayState, true);
        return windowInstance;
    }

    async hide() {
        const windowInstance = await this.resolveExistingWindow();
        if (!windowInstance) {
            return;
        }

        await windowInstance.hide();
    }

    async close() {
        const windowInstance = await this.resolveExistingWindow();
        if (!windowInstance) {
            return;
        }

        await windowInstance.close();
        if (this.windowInstance === windowInstance) {
            this.windowInstance = null;
        }
    }

    async getWindow() {
        return await this.resolveExistingWindow();
    }

    async commitState(payload: T) {
        this.lastState = payload;
        await auxWindowStateService.set(this.options.label, payload);

        try {
            await emitTo(this.options.label, this.options.eventName, payload);
        } catch (error) {
            logger.debug('[AuxWindowController] State emit skipped or failed', {
                label: this.options.label,
                eventName: this.options.eventName,
                error,
            });
        }
    }

    async getState() {
        if (this.lastState) {
            return this.lastState;
        }

        const snapshot = await auxWindowStateService.get<T>(this.options.label);
        this.lastState = snapshot;
        return snapshot;
    }

    async clearState() {
        this.lastState = null;
        await auxWindowStateService.clear(this.options.label);
    }

    getLastState() {
        return this.lastState;
    }
}
