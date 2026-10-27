import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
  const emitTo = vi.fn().mockResolvedValue(undefined);
  const invoke = vi.fn().mockResolvedValue(undefined);
  const getByLabel = vi.fn().mockResolvedValue(null);
  const availableMonitors = vi.fn().mockResolvedValue([]);
  const primaryMonitor = vi.fn().mockResolvedValue(null);
  const currentMonitor = vi.fn().mockResolvedValue(null);
  const createdWindows: any[] = [];
  const moveListeners: Array<(event: { payload: { x: number; y: number } }) => void> = [];
  const nextWindowFailures = {
    setAlwaysOnTop: null as unknown,
    setIgnoreCursorEvents: null as unknown,
  };

  class MockPhysicalSize {
    constructor(
      public width: number,
      public height: number
    ) {}
  }

  class MockPhysicalPosition {
    constructor(
      public x: number,
      public y: number
    ) {}
  }

  const buildWindow = () => {
    const windowInstance = {
      once: vi.fn((event: string, callback: (payload?: unknown) => void) => {
        if (event === 'tauri://created') {
          callback();
        }
        return Promise.resolve(undefined);
      }),
      close: vi.fn().mockResolvedValue(undefined),
      hide: vi.fn().mockResolvedValue(undefined),
      show: vi.fn().mockResolvedValue(undefined),
      setFocus: vi.fn().mockResolvedValue(undefined),
      setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
      setIgnoreCursorEvents: vi.fn().mockResolvedValue(undefined),
      scaleFactor: vi.fn().mockResolvedValue(1),
      innerSize: vi.fn().mockResolvedValue({ width: 800, height: 120 }),
      setSize: vi.fn().mockResolvedValue(undefined),
      setPosition: vi.fn().mockResolvedValue(undefined),
      onMoved: vi.fn((cb: (event: { payload: { x: number; y: number } }) => void) => {
        moveListeners.push(cb);
        return Promise.resolve(vi.fn());
      }),
    };
    if (nextWindowFailures.setAlwaysOnTop) {
      windowInstance.setAlwaysOnTop.mockRejectedValueOnce(nextWindowFailures.setAlwaysOnTop);
    }
    if (nextWindowFailures.setIgnoreCursorEvents) {
      windowInstance.setIgnoreCursorEvents.mockRejectedValueOnce(
        nextWindowFailures.setIgnoreCursorEvents
      );
    }

    return windowInstance;
  };

  class MockWebviewWindow {
    label: string;
    options: Record<string, unknown>;
    windowInstance: ReturnType<typeof buildWindow>;

    constructor(label: string, options: Record<string, unknown>) {
      this.label = label;
      this.options = options;
      this.windowInstance = buildWindow();
      createdWindows.push(this);
      return this.windowInstance as any;
    }

    static getByLabel(label: string) {
      return getByLabel(label);
    }
  }

  return {
    MockPhysicalPosition,
    MockPhysicalSize,
    MockWebviewWindow,
    availableMonitors,
    buildWindow,
    createdWindows,
    currentMonitor,
    emitTo,
    getByLabel,
    invoke,
    moveListeners,
    nextWindowFailures,
    primaryMonitor,
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  emitTo: mocks.emitTo,
}));

vi.mock('@tauri-apps/api/dpi', () => ({
  PhysicalPosition: mocks.MockPhysicalPosition,
  PhysicalSize: mocks.MockPhysicalSize,
}));

vi.mock('@tauri-apps/api/window', () => ({
  availableMonitors: mocks.availableMonitors,
  primaryMonitor: mocks.primaryMonitor,
  currentMonitor: mocks.currentMonitor,
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: mocks.MockWebviewWindow,
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  },
}));

const defaultStyle = {
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
  textAlign: 'center' as const,
  backgroundBlur: true,
  backgroundBlurRadius: 20,
  disappearDelay: 4.0,
  displayPriority: 'translation_main' as const,
};

describe('CaptionWindowService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mocks.createdWindows.length = 0;
    mocks.moveListeners.length = 0;
    mocks.getByLabel.mockResolvedValue(null);
    mocks.availableMonitors.mockResolvedValue([]);
    mocks.primaryMonitor.mockResolvedValue(null);
    mocks.currentMonitor.mockResolvedValue(null);
    mocks.invoke.mockResolvedValue(undefined);
    mocks.emitTo.mockResolvedValue(undefined);
    mocks.nextWindowFailures.setAlwaysOnTop = null;
    mocks.nextWindowFailures.setIgnoreCursorEvents = null;
  });

  it('opens the window with the default caption state and shared transport payload', async () => {
    const { captionWindowService } = await import('../captionWindowService');

    await captionWindowService.open();

    expect(mocks.createdWindows).toHaveLength(1);
    expect(mocks.createdWindows[0].options).toEqual(
      expect.objectContaining({
        url: '/index.html?window=caption',
        x: undefined,
        y: undefined,
        width: 800,
        height: 120,
        resizable: true,
        maximizable: false,
        minimizable: false,
        transparent: true,
        decorations: false,
        alwaysOnTop: true,
        visible: true,
      })
    );
    expect(mocks.createdWindows[0].windowInstance.setPosition).toHaveBeenCalledWith(
      expect.objectContaining({ x: 560, y: 932 })
    );
    expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
      label: 'caption',
      payload: {
        revision: 1,
        segments: [],
        style: defaultStyle,
      },
    });
    expect(mocks.emitTo).toHaveBeenCalledWith(
      {
        kind: 'AnyLabel',
        label: 'caption',
      },
      'caption:state',
      {
        revision: 1,
        segments: [],
        style: defaultStyle,
      }
    );
  });

  it('opens the window with custom style properties and applies window flags', async () => {
    const { captionWindowService } = await import('../captionWindowService');

    await captionWindowService.open({
      width: 1000,
      fontSize: 32,
      color: '#ff0000',
      backgroundColor: '#112233',
      backgroundOpacity: 0.75,
      alwaysOnTop: false,
      lockWindow: true,
    });

    const createdWindow = mocks.createdWindows[0].windowInstance;

    expect(mocks.createdWindows[0].options).toEqual(
      expect.objectContaining({
        url: '/index.html?window=caption',
        width: 1000,
        alwaysOnTop: false,
      })
    );
    expect(createdWindow.setAlwaysOnTop).not.toHaveBeenCalled();
    expect(createdWindow.setIgnoreCursorEvents).toHaveBeenCalledWith(true);
    expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
      label: 'caption',
      payload: {
        revision: 1,
        segments: [],
        style: {
          ...defaultStyle,
          width: 1000,
          fontSize: 32,
          color: '#ff0000',
          backgroundColor: '#112233',
          backgroundOpacity: 0.75,
        },
      },
    });
  });

  it('keeps opening when post-create caption window flags hit a stale window handle', async () => {
    mocks.nextWindowFailures.setIgnoreCursorEvents = new Error('window not found');

    const { captionWindowService } = await import('../captionWindowService');

    await expect(
      captionWindowService.open({
        lockWindow: true,
      })
    ).resolves.toBeUndefined();

    expect(mocks.createdWindows).toHaveLength(1);
    expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
      label: 'caption',
      payload: {
        revision: 1,
        segments: [],
        style: defaultStyle,
      },
    });
    expect(mocks.createdWindows[0].windowInstance.setIgnoreCursorEvents).toHaveBeenCalledWith(true);
  });

  it('ignores stale-window failures when applying caption window flags later', async () => {
    const existingWindow = mocks.buildWindow();
    existingWindow.setAlwaysOnTop.mockRejectedValueOnce(new Error('window not found'));
    existingWindow.setIgnoreCursorEvents.mockRejectedValueOnce(new Error('window not found'));
    mocks.getByLabel.mockResolvedValue(existingWindow);

    const { captionWindowService } = await import('../captionWindowService');

    await expect(captionWindowService.setAlwaysOnTop(false)).resolves.toBeUndefined();
    await expect(captionWindowService.setClickThrough(true)).resolves.toBeUndefined();

    expect(existingWindow.setAlwaysOnTop).toHaveBeenCalledWith(false);
    expect(existingWindow.setIgnoreCursorEvents).toHaveBeenCalledWith(true);
  });

  it('reuses an existing window and keeps style updates flowing through shared state without resizing from the service', async () => {
    const existingWindow = mocks.buildWindow();
    mocks.getByLabel.mockResolvedValue(existingWindow);

    const { captionWindowService } = await import('../captionWindowService');

    await captionWindowService.open({ width: 1200, fontSize: 40 });

    expect(mocks.createdWindows).toHaveLength(0);
    expect(existingWindow.show).toHaveBeenCalled();
    expect(mocks.invoke).toHaveBeenCalledWith('focus_window', { label: 'caption' });
    expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
      label: 'caption',
      payload: {
        revision: 1,
        segments: [],
        style: {
          ...defaultStyle,
          width: 1200,
          fontSize: 40,
        },
      },
    });

    vi.clearAllMocks();
    mocks.getByLabel.mockResolvedValue(existingWindow);

    await captionWindowService.updateStyle({ width: 1400, fontSize: 42 });

    expect(mocks.invoke).toHaveBeenCalledWith('set_aux_window_state', {
      label: 'caption',
      payload: {
        revision: 2,
        segments: [],
        style: {
          ...defaultStyle,
          width: 1400,
          fontSize: 42,
        },
      },
    });
    expect(existingWindow.setSize).not.toHaveBeenCalled();
  });

  it('skips committing style updates when the caption style is unchanged', async () => {
    const { captionWindowService } = await import('../captionWindowService');

    await captionWindowService.updateStyle({
      ...defaultStyle,
    });

    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(mocks.emitTo).not.toHaveBeenCalled();
    expect(mocks.getByLabel).not.toHaveBeenCalled();
  });
  it('resolves default bottom-center position using primary monitor workArea', async () => {
    mocks.primaryMonitor.mockResolvedValue({
      scaleFactor: 1.5,
      position: { x: 0, y: 0 },
      size: { width: 3840, height: 2160 },
      workArea: {
        position: { x: 0, y: 0 },
        size: { width: 3840, height: 2100 },
      },
    });

    const { resolveCaptionWindowInitialPosition } = await import('../captionWindowService');
    const [x, y] = await resolveCaptionWindowInitialPosition(800, 120);

    // width 800 * 1.5 = 1200. x = (3840 - 1200) / 2 = 1320
    // height 120 * 1.5 = 180. y = 2100 - 180 - round(28 * 1.5 = 42) = 1878
    expect(x).toBe(1320);
    expect(y).toBe(1878);
  });

  it('clamps out-of-bounds coordinates to default bottom-center and accepts in-bounds coordinates', async () => {
    const mockMonitor = {
      scaleFactor: 1.0,
      position: { x: 0, y: 0 },
      size: { width: 1920, height: 1080 },
      workArea: {
        position: { x: 0, y: 0 },
        size: { width: 1920, height: 1040 },
      },
    };
    mocks.availableMonitors.mockResolvedValue([mockMonitor]);
    mocks.primaryMonitor.mockResolvedValue(mockMonitor);

    const { resolveCaptionWindowInitialPosition } = await import('../captionWindowService');

    // Valid in-bounds coordinates
    const [inX, inY] = await resolveCaptionWindowInitialPosition(800, 120, 200, 300);
    expect(inX).toBe(200);
    expect(inY).toBe(300);

    // Out of bounds coordinates (-500, 9999)
    const [outX, outY] = await resolveCaptionWindowInitialPosition(800, 120, -500, 9999);
    // Should fall back to default bottom-center: x = (1920 - 800)/2 = 560, y = 1040 - 120 - 28 = 892
    expect(outX).toBe(560);
    expect(outY).toBe(892);
  });

  it('tracks window drag and persists coordinates to configStore with debounce', async () => {
    vi.useFakeTimers();
    mocks.getByLabel.mockResolvedValue(null);

    // Module reset in beforeEach requires dynamic import to bind freshly hoisted mocks
    const { captionWindowService } = await import('../captionWindowService');
    const { useConfigStore } = await import('../../stores/configStore');

    // Open caption window
    await captionWindowService.open();
    expect(mocks.moveListeners).toHaveLength(1);

    // Directly trigger moveListener simulating drag
    mocks.moveListeners[0]({ payload: { x: 350, y: 450 } });

    // Before debounce
    expect(useConfigStore.getState().config.captionWindowX).toBeNull();

    // Advance timers past 300ms debounce
    vi.advanceTimersByTime(350);

    expect(useConfigStore.getState().config.captionWindowX).toBe(350);
    expect(useConfigStore.getState().config.captionWindowY).toBe(450);

    vi.useRealTimers();
  });

  it('resetPosition clears saved config coordinates and repositions active window', async () => {
    const existingWindow = mocks.buildWindow();
    mocks.getByLabel.mockResolvedValue(existingWindow);

    const { captionWindowService } = await import('../captionWindowService');
    const { useConfigStore } = await import('../../stores/configStore');

    useConfigStore.getState().setConfig({
      captionWindowX: 700,
      captionWindowY: 800,
    });

    await captionWindowService.resetPosition();

    expect(useConfigStore.getState().config.captionWindowX).toBeNull();
    expect(useConfigStore.getState().config.captionWindowY).toBeNull();
    expect(existingWindow.setPosition).toHaveBeenCalledWith(
      expect.objectContaining({ x: 560, y: 932 })
    );
  });

  it('does not overwrite position when opening an existing caption window without coordinates', async () => {
    const existingWindow = mocks.buildWindow();
    mocks.getByLabel.mockResolvedValue(existingWindow);

    const { captionWindowService } = await import('../captionWindowService');

    await captionWindowService.open({ width: 900 });

    expect(existingWindow.setPosition).not.toHaveBeenCalled();
  });

  it('ignores initial default coordinates in onMoved and persists user dragging', async () => {
    vi.useFakeTimers();
    mocks.getByLabel.mockResolvedValue(null);

    const { captionWindowService } = await import('../captionWindowService');
    const { useConfigStore } = await import('../../stores/configStore');

    useConfigStore.getState().setConfig({
      captionWindowX: null,
      captionWindowY: null,
    });

    await captionWindowService.open();
    expect(mocks.moveListeners).toHaveLength(1);

    // Trigger onMoved with initial default coordinates [560, 932]
    mocks.moveListeners[0]({ payload: { x: 560, y: 932 } });
    vi.advanceTimersByTime(350);

    // Initial position was ignored, not persisted
    expect(useConfigStore.getState().config.captionWindowX).toBeNull();
    expect(useConfigStore.getState().config.captionWindowY).toBeNull();

    // Trigger onMoved with user drag coordinates
    mocks.moveListeners[0]({ payload: { x: 400, y: 500 } });
    vi.advanceTimersByTime(350);

    // Dragged position was persisted
    expect(useConfigStore.getState().config.captionWindowX).toBe(400);
    expect(useConfigStore.getState().config.captionWindowY).toBe(500);

    vi.useRealTimers();
  });
  it('persists moving the window back to default coordinates', async () => {
    vi.useFakeTimers();
    mocks.getByLabel.mockResolvedValue(null);

    const { captionWindowService } = await import('../captionWindowService');
    const { useConfigStore } = await import('../../stores/configStore');

    useConfigStore.getState().setConfig({
      captionWindowX: null,
      captionWindowY: null,
    });

    await captionWindowService.open();
    expect(mocks.moveListeners).toHaveLength(1);

    // Initial position event matches programmatic target, suppressed
    mocks.moveListeners[0]({ payload: { x: 560, y: 932 } });
    vi.advanceTimersByTime(350);
    expect(useConfigStore.getState().config.captionWindowX).toBeNull();
    expect(useConfigStore.getState().config.captionWindowY).toBeNull();

    // Trigger onMoved with user drag coordinates
    mocks.moveListeners[0]({ payload: { x: 400, y: 500 } });
    vi.advanceTimersByTime(350);
    expect(useConfigStore.getState().config.captionWindowX).toBe(400);
    expect(useConfigStore.getState().config.captionWindowY).toBe(500);

    // User moves back to default coordinates [560, 932]
    mocks.moveListeners[0]({ payload: { x: 560, y: 932 } });
    vi.advanceTimersByTime(350);
    expect(useConfigStore.getState().config.captionWindowX).toBe(560);
    expect(useConfigStore.getState().config.captionWindowY).toBe(932);

    vi.useRealTimers();
  });

  it('resolves negative multi-monitor coordinates when inside secondary monitor bounds', async () => {
    const primaryMonitor = {
      scaleFactor: 1.0,
      position: { x: 0, y: 0 },
      size: { width: 1920, height: 1080 },
      workArea: {
        position: { x: 0, y: 0 },
        size: { width: 1920, height: 1040 },
      },
    };
    const leftMonitor = {
      scaleFactor: 1.0,
      position: { x: -1920, y: 0 },
      size: { width: 1920, height: 1080 },
      workArea: {
        position: { x: -1920, y: 0 },
        size: { width: 1920, height: 1040 },
      },
    };
    mocks.availableMonitors.mockResolvedValue([leftMonitor, primaryMonitor]);
    mocks.primaryMonitor.mockResolvedValue(primaryMonitor);

    const { resolveCaptionWindowInitialPosition } = await import('../captionWindowService');

    // Valid negative coordinates on the left monitor
    const [x, y] = await resolveCaptionWindowInitialPosition(800, 120, -1000, 500);
    expect(x).toBe(-1000);
    expect(y).toBe(500);
  });
});
