import { useConfigStore } from '../stores/configStore';
import type { TranscriptSegment } from '../types/transcript';
import { logger } from '../utils/logger';
import {
  AuxWindowController,
  type AuxWindowControllerOptions,
  type AuxWindowDisplayState,
} from './auxWindowController';
import { TauriEvent } from './tauri/events';
import {
  availableMonitors,
  currentMonitor,
  PhysicalPosition,
  primaryMonitor,
  WebviewWindow,
} from './tauri/platform/windows';

export const CAPTION_WINDOW_LABEL = 'caption';
export const CAPTION_EVENT_STATE = TauriEvent.auxWindow.captionState;
const CAPTION_INITIAL_HEIGHT = 120;

export interface CaptionWindowStyle {
  width: number;
  fontSize: number;
  color: string;
  backgroundColor: string;
  backgroundOpacity: number;
  displayMode?: 'bilingual' | 'translation' | 'original';
  bilingualOrder?: 'original_first' | 'translation_first';
  translationColor?: string;
  translationFontSize?: number;
  displayPriority?: 'translation_main' | 'original_main' | 'translation_only' | 'original_only';
  textStroke?: boolean;
  strokeWidth?: number;
  strokeColor?: string;
  textShadow?: boolean;
  textAlign?: 'center' | 'left';
  backgroundBlur?: boolean;
  backgroundBlurRadius?: number;
  disappearDelay?: number;
}

export interface CaptionWindowOpenOptions {
  x?: number | null;
  y?: number | null;
  alwaysOnTop?: boolean;
  lockWindow?: boolean;
  width?: number;
  fontSize?: number;
  color?: string;
  backgroundColor?: string;
  backgroundOpacity?: number;
  displayMode?: 'bilingual' | 'translation' | 'original';
  bilingualOrder?: 'original_first' | 'translation_first';
  translationColor?: string;
  translationFontSize?: number;
  displayPriority?: 'translation_main' | 'original_main' | 'translation_only' | 'original_only';
  textStroke?: boolean;
  strokeWidth?: number;
  strokeColor?: string;
  textShadow?: boolean;
  textAlign?: 'center' | 'left';
  backgroundBlur?: boolean;
  backgroundBlurRadius?: number;
  disappearDelay?: number;
}

export interface CaptionWindowState {
  revision: number;
  segments: TranscriptSegment[];
  style: CaptionWindowStyle;
}

const DEFAULT_CAPTION_STYLE: CaptionWindowStyle = {
  width: 800,
  fontSize: 22,
  color: '#ffffff',
  backgroundColor: '#17171a',
  backgroundOpacity: 0.72,
  translationColor: '#6ee7b7',
  translationFontSize: 16,
  textStroke: false,
  strokeWidth: 0,
  strokeColor: '#000000',
  textShadow: true,
  textAlign: 'center',
  backgroundBlur: true,
  backgroundBlurRadius: 20,
  disappearDelay: 4.0,
  displayPriority: 'translation_main',
};

export const DEFAULT_CAPTION_WINDOW_STATE: CaptionWindowState = {
  revision: 0,
  segments: [],
  style: DEFAULT_CAPTION_STYLE,
};

function areCaptionStylesEqual(left: CaptionWindowStyle, right: CaptionWindowStyle): boolean {
  return (
    left.width === right.width &&
    left.fontSize === right.fontSize &&
    left.color === right.color &&
    left.backgroundColor === right.backgroundColor &&
    left.backgroundOpacity === right.backgroundOpacity &&
    left.displayMode === right.displayMode &&
    left.bilingualOrder === right.bilingualOrder &&
    left.translationColor === right.translationColor &&
    left.translationFontSize === right.translationFontSize &&
    left.displayPriority === right.displayPriority &&
    left.textStroke === right.textStroke &&
    left.strokeWidth === right.strokeWidth &&
    left.strokeColor === right.strokeColor &&
    left.textShadow === right.textShadow &&
    left.textAlign === right.textAlign &&
    left.backgroundBlur === right.backgroundBlur &&
    left.backgroundBlurRadius === right.backgroundBlurRadius &&
    left.disappearDelay === right.disappearDelay
  );
}

function isWindowNotFoundError(error: unknown): boolean {
  if (error instanceof Error) {
    return error.message.toLowerCase().includes('window not found');
  }

  return typeof error === 'string' && error.toLowerCase().includes('window not found');
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
export async function resolveCaptionWindowInitialPosition(
  width: number,
  height: number,
  savedX?: number | null,
  savedY?: number | null
): Promise<[number, number]> {
  const monitors = await availableMonitors().catch(() => []);
  const monitor =
    (await primaryMonitor().catch(() => null)) ??
    (await currentMonitor().catch(() => null)) ??
    monitors[0] ??
    null;

  const scale = monitor?.scaleFactor ?? 1;
  const workArea =
    monitor?.workArea?.position && monitor?.workArea?.size
      ? {
          x: monitor.workArea.position.x,
          y: monitor.workArea.position.y,
          width: monitor.workArea.size.width,
          height: monitor.workArea.size.height,
        }
      : monitor?.position && monitor?.size
        ? {
            x: monitor.position.x,
            y: monitor.position.y,
            width: monitor.size.width,
            height: monitor.size.height,
          }
        : { x: 0, y: 0, width: 1920, height: 1080 };

  const physWidth = Math.ceil(width * scale);
  const physHeight = Math.ceil(height * scale);
  const defaultX = Math.round(workArea.x + (workArea.width - physWidth) / 2);
  const defaultY = Math.round(workArea.y + workArea.height - physHeight - Math.round(28 * scale));

  if (savedX != null && savedY != null) {
    const candidates = monitors.length > 0 ? monitors : monitor ? [monitor] : [];
    if (candidates.length > 0) {
      const isInsideAnyMonitor = candidates.some((m) => {
        if (
          m?.position?.x == null ||
          m?.position?.y == null ||
          m?.size?.width == null ||
          m?.size?.height == null
        ) {
          return false;
        }
        const minX = m.position.x;
        const maxX = m.position.x + m.size.width;
        const minY = m.position.y;
        const maxY = m.position.y + m.size.height;
        return savedX >= minX && savedX < maxX && savedY >= minY && savedY < maxY;
      });

      if (isInsideAnyMonitor) {
        return [savedX, savedY];
      }
    } else if (Number.isFinite(savedX) && Number.isFinite(savedY)) {
      return [savedX, savedY];
    }
  }

  return [defaultX, defaultY];
}

export interface CaptionWindowServicePorts {
  createAuxWindowController: (
    options: AuxWindowControllerOptions
  ) => AuxWindowController<CaptionWindowState>;
  createWebviewWindow: (
    label: string,
    options: ConstructorParameters<typeof WebviewWindow>[1]
  ) => WebviewWindow;
}

export class CaptionWindowService {
  private controller: AuxWindowController<CaptionWindowState>;
  private state: CaptionWindowState = DEFAULT_CAPTION_WINDOW_STATE;
  private moveUnlisten: (() => void) | null = null;
  private moveTimer: number | null = null;
  private isResettingPosition = false;
  private isBindingMoveListener = false;

  constructor(private readonly ports: CaptionWindowServicePorts) {
    this.controller = this.ports.createAuxWindowController({
      label: CAPTION_WINDOW_LABEL,
      eventName: CAPTION_EVENT_STATE,
      createWindow: (displayState: AuxWindowDisplayState, creationState: { visible: boolean }) =>
        this.ports.createWebviewWindow(CAPTION_WINDOW_LABEL, {
          url: '/index.html?window=caption',
          title: 'Sona Live Caption',
          alwaysOnTop: displayState.alwaysOnTop ?? true,
          decorations: false,
          transparent: true,
          skipTaskbar: true,
          x: undefined,
          y: undefined,
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
  }

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

  async open(options?: CaptionWindowOpenOptions) {
    logger.info('[CaptionWindowService] Opening caption window', {
      width: options?.width ?? this.state.style.width,
      fontSize: options?.fontSize ?? this.state.style.fontSize,
      color: options?.color ?? this.state.style.color,
      backgroundColor: options?.backgroundColor ?? this.state.style.backgroundColor,
      backgroundOpacity: options?.backgroundOpacity ?? this.state.style.backgroundOpacity,
      alwaysOnTop: options?.alwaysOnTop ?? true,
      lockWindow: options?.lockWindow ?? false,
    });

    const nextStyle: CaptionWindowStyle = {
      ...this.state.style,
      ...(options?.width !== undefined ? { width: options.width } : {}),
      ...(options?.fontSize !== undefined ? { fontSize: options.fontSize } : {}),
      ...(options?.color !== undefined ? { color: options.color } : {}),
      ...(options?.backgroundColor !== undefined
        ? { backgroundColor: options.backgroundColor }
        : {}),
      ...(options?.backgroundOpacity !== undefined
        ? { backgroundOpacity: options.backgroundOpacity }
        : {}),
      ...(options?.displayMode !== undefined ? { displayMode: options.displayMode } : {}),
      ...(options?.bilingualOrder !== undefined ? { bilingualOrder: options.bilingualOrder } : {}),
      ...(options?.translationColor !== undefined
        ? { translationColor: options.translationColor }
        : {}),
      ...(options?.translationFontSize !== undefined
        ? { translationFontSize: options.translationFontSize }
        : {}),
      ...(options?.displayPriority !== undefined
        ? { displayPriority: options.displayPriority }
        : {}),
      ...(options?.textStroke !== undefined ? { textStroke: options.textStroke } : {}),
      ...(options?.strokeWidth !== undefined ? { strokeWidth: options.strokeWidth } : {}),
      ...(options?.strokeColor !== undefined ? { strokeColor: options.strokeColor } : {}),
      ...(options?.textShadow !== undefined ? { textShadow: options.textShadow } : {}),
      ...(options?.textAlign !== undefined ? { textAlign: options.textAlign } : {}),
      ...(options?.backgroundBlur !== undefined ? { backgroundBlur: options.backgroundBlur } : {}),
      ...(options?.backgroundBlurRadius !== undefined
        ? { backgroundBlurRadius: options.backgroundBlurRadius }
        : {}),
      ...(options?.disappearDelay !== undefined ? { disappearDelay: options.disappearDelay } : {}),
    };

    await this.commitState({ style: nextStyle });

    const requestedAlwaysOnTop = options?.alwaysOnTop;
    const requestedLockWindow = options?.lockWindow;
    const [targetX, targetY] = await resolveCaptionWindowInitialPosition(
      nextStyle.width,
      CAPTION_INITIAL_HEIGHT,
      options?.x,
      options?.y
    );

    const hadExistingWindow = Boolean(await this.controller.getWindow());
    const shouldSetPosition = !hadExistingWindow || options?.x != null || options?.y != null;
    const windowInstance = await this.controller.open({
      size: { width: nextStyle.width, height: CAPTION_INITIAL_HEIGHT },
      ...(shouldSetPosition ? { position: [targetX, targetY] } : {}),
      focus: true,
      alwaysOnTop: requestedAlwaysOnTop ?? true,
    });

    if (!windowInstance) {
      logger.error('[CaptionWindowService] Failed to obtain caption window handle');
      return;
    }

    if (!hadExistingWindow) {
      await windowInstance.setPosition(new PhysicalPosition(targetX, targetY)).catch(() => {});
    }

    if (
      !this.moveUnlisten &&
      !this.isBindingMoveListener &&
      typeof windowInstance.onMoved === 'function'
    ) {
      this.isBindingMoveListener = true;
      try {
        this.moveUnlisten = await windowInstance.onMoved(({ payload: position }) => {
          if (this.isResettingPosition) return;
          if (
            options?.x == null &&
            options?.y == null &&
            Math.abs(position.x - targetX) < 2 &&
            Math.abs(position.y - targetY) < 2
          ) {
            return;
          }
          clearTimeout(this.moveTimer ?? undefined);
          this.moveTimer = window.setTimeout(() => {
            useConfigStore.getState().setConfig({
              captionWindowX: position.x,
              captionWindowY: position.y,
            });
          }, 300);
        });

        windowInstance
          .once('tauri://destroyed', () => {
            clearTimeout(this.moveTimer ?? undefined);
            this.moveTimer = null;
            if (this.moveUnlisten) {
              this.moveUnlisten();
              this.moveUnlisten = null;
            }
          })
          .catch(() => {});
      } catch (err) {
        logger.warn('[CaptionWindowService] Failed to bind window move listener', { err });
      } finally {
        this.isBindingMoveListener = false;
      }
    }

    if (requestedAlwaysOnTop !== undefined && hadExistingWindow) {
      await applyCaptionWindowFlag('alwaysOnTop', () =>
        windowInstance.setAlwaysOnTop(requestedAlwaysOnTop)
      );
    }

    if (requestedLockWindow !== undefined) {
      await applyCaptionWindowFlag('lockWindow', () =>
        windowInstance.setIgnoreCursorEvents(requestedLockWindow)
      );
    }

    logger.info('[CaptionWindowService] Caption window open request completed');
  }

  async close() {
    logger.info('[CaptionWindowService] Requested to close caption window');
    clearTimeout(this.moveTimer ?? undefined);
    this.moveTimer = null;
    this.isBindingMoveListener = false;
    if (this.moveUnlisten) {
      this.moveUnlisten();
      this.moveUnlisten = null;
    }
    this.state = {
      ...this.state,
      revision: this.state.revision + 1,
      segments: [],
    };
    await this.controller.clearState();
    await this.controller.close();
  }

  async resetPosition() {
    this.isResettingPosition = true;
    clearTimeout(this.moveTimer ?? undefined);
    this.moveTimer = null;

    const [defaultX, defaultY] = await resolveCaptionWindowInitialPosition(
      this.state.style.width,
      CAPTION_INITIAL_HEIGHT,
      null,
      null
    );

    useConfigStore.getState().setConfig({
      captionWindowX: null,
      captionWindowY: null,
    });

    const windowInstance = await this.controller.getWindow();
    if (windowInstance) {
      await windowInstance.setPosition(new PhysicalPosition(defaultX, defaultY)).catch(() => {});
    }

    window.setTimeout(() => {
      this.isResettingPosition = false;
    }, 500);
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
      await applyCaptionWindowFlag('alwaysOnTop', () => windowInstance.setAlwaysOnTop(enabled));
    }
  }

  async setClickThrough(enabled: boolean) {
    const windowInstance = await this.controller.getWindow();
    if (windowInstance) {
      await applyCaptionWindowFlag('clickThrough', () =>
        windowInstance.setIgnoreCursorEvents(enabled)
      );
    }
  }

  async updateStyle(style: Partial<CaptionWindowStyle>) {
    const nextStyle: CaptionWindowStyle = {
      ...this.state.style,
      ...(style.width !== undefined ? { width: style.width } : {}),
      ...(style.fontSize !== undefined ? { fontSize: style.fontSize } : {}),
      ...(style.color !== undefined ? { color: style.color } : {}),
      ...(style.backgroundColor !== undefined ? { backgroundColor: style.backgroundColor } : {}),
      ...(style.backgroundOpacity !== undefined
        ? { backgroundOpacity: style.backgroundOpacity }
        : {}),
      ...(style.displayMode !== undefined ? { displayMode: style.displayMode } : {}),
      ...(style.bilingualOrder !== undefined ? { bilingualOrder: style.bilingualOrder } : {}),
      ...(style.translationColor !== undefined ? { translationColor: style.translationColor } : {}),
      ...(style.translationFontSize !== undefined
        ? { translationFontSize: style.translationFontSize }
        : {}),
      ...(style.displayPriority !== undefined ? { displayPriority: style.displayPriority } : {}),
      ...(style.textStroke !== undefined ? { textStroke: style.textStroke } : {}),
      ...(style.strokeWidth !== undefined ? { strokeWidth: style.strokeWidth } : {}),
      ...(style.strokeColor !== undefined ? { strokeColor: style.strokeColor } : {}),
      ...(style.textShadow !== undefined ? { textShadow: style.textShadow } : {}),
      ...(style.textAlign !== undefined ? { textAlign: style.textAlign } : {}),
      ...(style.backgroundBlur !== undefined ? { backgroundBlur: style.backgroundBlur } : {}),
      ...(style.backgroundBlurRadius !== undefined
        ? { backgroundBlurRadius: style.backgroundBlurRadius }
        : {}),
      ...(style.disappearDelay !== undefined ? { disappearDelay: style.disappearDelay } : {}),
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

export function createCaptionWindowService(ports: CaptionWindowServicePorts): CaptionWindowService {
  return new CaptionWindowService(ports);
}

export const captionWindowService = createCaptionWindowService({
  createAuxWindowController: (options) => new AuxWindowController<CaptionWindowState>(options),
  createWebviewWindow: (label, options) => new WebviewWindow(label, options),
});
