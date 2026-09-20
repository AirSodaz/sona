import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
  const shortcutState: {
    handler?: (event: any) => void;
    handlers: Record<string, (event: any) => void>;
  } = { handlers: {} };
  const configEvents: {
    listener?: (state: { config: Record<string, any> }) => void;
  } = {};
  const eventListeners: Record<string, (event: any) => void> = {};
  return {
    shortcutState,
    configEvents,
    eventListeners,
    defaultConfig: {
      voiceTypingEnabled: false,
      voiceTypingShortcut: 'Alt+V',
      voiceTypingMode: 'hold',
      streamingModelPath: 'path/to/model',
      vadModelPath: 'path/to/vad',
      language: 'auto',
      enableITN: true,
      microphoneId: 'default',
      microphoneBoost: 1,
      keepMicrophoneActive: false,
    },
    config: {
      voiceTypingEnabled: false,
      voiceTypingShortcut: 'Alt+V',
      voiceTypingMode: 'hold',
      streamingModelPath: 'path/to/model',
      vadModelPath: 'path/to/vad',
      language: 'auto',
      enableITN: true,
      microphoneId: 'default',
      microphoneBoost: 1,
      keepMicrophoneActive: false,
    } as Record<string, any>,
    configSubscribe: vi.fn(),
    invoke: vi.fn(),
    register: vi.fn(async (shortcut: string, handler: (event: any) => void) => {
      shortcutState.handlers[shortcut] = handler;
      if (!shortcut.includes('Shift')) {
        shortcutState.handler = handler;
      }
    }),
    unregister: vi.fn(),
    isRegistered: vi.fn().mockResolvedValue(false),
    mockPrepare: vi.fn(),
    mockStart: vi.fn(),
    mockAttachNative: vi.fn(),
    mockSoftStop: vi.fn(),
    setModelPath: vi.fn(),
    setLanguage: vi.fn(),
    setEnableITN: vi.fn(),
    loggerInfo: vi.fn(),
    loggerWarn: vi.fn(),
    loggerError: vi.fn(),
    loggerDebug: vi.fn(),
    windowPrepare: vi.fn(),
    windowOpen: vi.fn(),
    windowClose: vi.fn(),
    windowSendState: vi.fn(),
    windowClearState: vi.fn(),
    monitorFromPoint: vi.fn(async (_x?: number, _y?: number) => ({
      scaleFactor: 1,
      workArea: {
        position: { x: 0, y: 0 },
        size: { width: 1920, height: 1080 },
      },
      position: { x: 0, y: 0 },
      size: { width: 1920, height: 1080 },
    })),
    polishVoiceTypingText: vi.fn(async (text: string, _opts?: any) => `[polished] ${text}`),
    transformSelectedText: vi.fn(
      async (selected: string, instruction: string, _opts?: any) =>
        `[transformed: ${selected}] ${instruction}`
    ),
    translateVoiceTypingText: vi.fn(
      async (text: string, targetLang: string, _opts?: unknown) =>
        `[translated to ${targetLang}] ${text}`
    ),
    listen: vi.fn(async (eventName: string, handler: (event: any) => void) => {
      eventListeners[eventName] = handler;
      return () => {
        delete eventListeners[eventName];
      };
    }),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
  register: mocks.register,
  unregister: mocks.unregister,
  isRegistered: mocks.isRegistered,
}));
vi.mock('../tauri/platform/events', () => ({
  listen: mocks.listen,
  emit: vi.fn(),
  emitTo: vi.fn(),
}));
vi.mock('../voiceTypingWindowService', () => ({
  VOICE_TYPING_WINDOW_WIDTH: 400,
  voiceTypingWindowService: {
    prepare: mocks.windowPrepare,
    open: mocks.windowOpen,
    close: mocks.windowClose,
    sendState: mocks.windowSendState,
    clearState: mocks.windowClearState,
  },
}));
vi.mock('../voiceTyping/voiceTypingPolishService', () => ({
  polishVoiceTypingText: (text: string, opts?: unknown) => mocks.polishVoiceTypingText(text, opts),
  transformSelectedText: (selected: string, instruction: string, opts?: unknown) =>
    mocks.transformSelectedText(selected, instruction, opts),
  translateVoiceTypingText: (text: string, targetLang: string, opts?: unknown) =>
    mocks.translateVoiceTypingText(text, targetLang, opts),
}));
vi.mock('../tauri/platform/windows', () => ({
  currentMonitor: vi.fn(async () => ({
    scaleFactor: 1,
    workArea: {
      position: { x: 0, y: 0 },
      size: { width: 1920, height: 1080 },
    },
    position: { x: 0, y: 0 },
    size: { width: 1920, height: 1080 },
  })),
  monitorFromPoint: (x: number, y: number) => mocks.monitorFromPoint(x, y),
  getCurrentWindow: vi.fn(),
  getCurrentWebviewWindow: vi.fn(),
  WebviewWindow: vi.fn(),
  PhysicalPosition: class {},
  PhysicalSize: class {},
}));

vi.mock('../transcriptionService', () => {
  class MockTranscriptionService {
    setModelPath = mocks.setModelPath;
    setLanguage = mocks.setLanguage;
    setEnableITN = mocks.setEnableITN;
    prepare = mocks.mockPrepare;
    prepareNativeStart = mocks.mockStart;
    attachPreparedNative = mocks.mockAttachNative;
    softStop = mocks.mockSoftStop;
    stop = vi.fn();
  }

  return {
    TranscriptionService: MockTranscriptionService,
    createTranscriptionService: () => new MockTranscriptionService(),
  };
});

vi.mock('../../stores/configStore', () => ({
  useConfigStore: {
    getState: vi.fn(() => ({
      config: mocks.config,
    })),
    subscribe: mocks.configSubscribe,
  },
}));

vi.mock('../../stores/effectiveConfigStore', () => ({
  getEffectiveConfigSnapshot: vi.fn(() => mocks.config),
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    info: mocks.loggerInfo,
    warn: mocks.loggerWarn,
    error: mocks.loggerError,
    debug: mocks.loggerDebug,
  },
}));

async function loadService() {
  const module = await import('../voiceTypingService');
  return module.voiceTypingService as any;
}

async function loadRuntimeStore() {
  return await import('../../stores/voiceTypingRuntimeStore');
}

async function flushMicrotasks(times = 4) {
  for (let i = 0; i < times; i += 1) {
    await Promise.resolve();
  }
}

function getInvokeCalls(command: string) {
  return mocks.invoke.mock.calls.filter(([calledCommand]) => calledCommand === command);
}

function emitConfigChange(patch: Record<string, any>) {
  mocks.config = {
    ...mocks.config,
    ...patch,
  };
  mocks.configEvents.listener?.({ config: mocks.config });
}

describe('voiceTypingService', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    vi.resetModules();
    Object.defineProperty(navigator, 'platform', {
      value: 'Win32',
      configurable: true,
    });
    Object.defineProperty(navigator, 'userAgent', {
      value: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
      configurable: true,
    });

    mocks.shortcutState.handler = undefined;
    mocks.shortcutState.handlers = {};
    for (const key of Object.keys(mocks.eventListeners)) {
      delete mocks.eventListeners[key];
    }
    mocks.configEvents.listener = undefined;
    mocks.config = { ...mocks.defaultConfig };
    mocks.configSubscribe.mockImplementation(
      (listener: (state: { config: Record<string, any> }) => void) => {
        mocks.configEvents.listener = listener;
        return () => undefined;
      }
    );
    mocks.isRegistered.mockResolvedValue(false);
    mocks.mockPrepare.mockResolvedValue(undefined);
    mocks.mockStart.mockResolvedValue(undefined);
    mocks.mockAttachNative.mockResolvedValue(undefined);
    mocks.mockSoftStop.mockResolvedValue(undefined);
    mocks.windowPrepare.mockResolvedValue(undefined);
    mocks.windowOpen.mockResolvedValue(undefined);
    mocks.windowClose.mockResolvedValue(undefined);
    mocks.windowSendState.mockResolvedValue(undefined);
    mocks.windowClearState.mockResolvedValue(undefined);
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_text_cursor_position') {
        return [120, 280];
      }
      if (command === 'get_mouse_position') {
        return [240, 320];
      }
      if (
        command === 'start_microphone_capture' ||
        command === 'inject_text' ||
        command === 'stop_microphone_capture'
      ) {
        return undefined;
      }
      return undefined;
    });
  });

  afterEach(async () => {
    vi.clearAllTimers();
    const service = await loadService();
    service.destroy();
  });

  it('pre-warms the model and overlay window during initialization if enabled', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      language: 'zh',
    };

    const service = await loadService();
    service.init();
    await flushMicrotasks();

    expect(mocks.mockPrepare).toHaveBeenCalled();
    expect(mocks.windowPrepare).toHaveBeenCalledWith([0, 0]);
  });

  it('pre-warms persistent microphone capture when keep microphone active is enabled', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      keepMicrophoneActive: true,
    };

    const service = await loadService();
    service.init();
    await flushMicrotasks(8);

    expect(getInvokeCalls('start_microphone_capture')).toEqual([
      [
        'start_microphone_capture',
        {
          deviceName: null,
          instanceId: 'voice-typing',
        },
      ],
    ]);
  });

  it('skips persistent microphone capture during warm-up by default', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(mocks.mockPrepare).toHaveBeenCalled();
    expect(mocks.windowPrepare).toHaveBeenCalledWith([0, 0]);
    expect(getInvokeCalls('start_microphone_capture')).toEqual([]);
    expect(useVoiceTypingRuntimeStore.getState().warmup).toBe('ready');
  });

  it('passes the configured microphone boost into native voice typing inference', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      microphoneBoost: 2.5,
    };

    const service = await loadService();
    await service.startListening();

    expect(mocks.mockAttachNative).toHaveBeenCalledWith(
      expect.objectContaining({
        sourceKind: 'microphone',
        deviceName: null,
        gain: 2.5,
      })
    );
  });

  it('releases on-demand microphone capture after voice typing stops when persistence is disabled', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      keepMicrophoneActive: false,
    };

    const service = await loadService();

    await service.startListening();
    vi.clearAllMocks();

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;
    await flushMicrotasks(8);

    expect(getInvokeCalls('stop_microphone_capture')).toEqual([
      ['stop_microphone_capture', { instanceId: 'voice-typing' }],
    ]);
  });

  it('stops existing persistent microphone capture when the global persistence option is disabled', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      keepMicrophoneActive: true,
    };

    const service = await loadService();
    service.init();
    await flushMicrotasks(8);
    vi.clearAllMocks();

    emitConfigChange({ keepMicrophoneActive: false });
    await flushMicrotasks(8);

    expect(getInvokeCalls('stop_microphone_capture')).toEqual([
      ['stop_microphone_capture', { instanceId: 'voice-typing' }],
    ]);
  });

  it('keeps an active dictation capture running until stop when persistence is disabled mid-session', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      keepMicrophoneActive: true,
    };

    const service = await loadService();
    service.init();
    await flushMicrotasks(8);
    await service.startListening();
    vi.clearAllMocks();

    emitConfigChange({ keepMicrophoneActive: false });
    await flushMicrotasks(8);

    expect(getInvokeCalls('stop_microphone_capture')).toEqual([]);

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;

    expect(getInvokeCalls('stop_microphone_capture')).toEqual([
      ['stop_microphone_capture', { instanceId: 'voice-typing' }],
    ]);
  });

  it('updates runtime status to ready after successful shortcut registration and warm-up', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'ready',
        warmup: 'ready',
        lastErrorSource: null,
        lastErrorMessage: null,
      })
    );
  });

  it('records shortcut registration failures in the runtime status store', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };
    mocks.register.mockRejectedValueOnce(new Error('Shortcut is already in use.'));

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'error',
        lastErrorSource: 'shortcut_registration',
        lastErrorMessage: 'Shortcut is already in use.',
      })
    );
  });

  it('normalizes Tauri-shaped shortcut registration failures in runtime status', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };
    mocks.register.mockRejectedValueOnce(
      new Error(
        'error invoking command: register_shortcut\nCaused by: Shortcut already registered by another app.'
      )
    );

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'error',
        lastErrorSource: 'shortcut_registration',
        lastErrorMessage: 'Shortcut already registered by another app.',
      })
    );
  });

  it('records warm-up failures in the runtime status store', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };
    mocks.mockPrepare.mockRejectedValueOnce(new Error('Warm-up failed.'));

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        warmup: 'error',
        lastErrorSource: 'warmup',
        lastErrorMessage: 'Warm-up failed.',
      })
    );
  });

  it('normalizes object-shaped warm-up failures in runtime status', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };
    mocks.mockPrepare.mockRejectedValueOnce({
      code: 'E_WARMUP',
      error: {
        message: 'Nested warm-up failed.',
      },
    });

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        warmup: 'error',
        lastErrorSource: 'warmup',
        lastErrorMessage: 'Nested warm-up failed.',
      })
    );
  });

  it('records microphone pre-warm failures in the runtime status store', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      keepMicrophoneActive: true,
    };
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_text_cursor_position') {
        return [120, 280];
      }
      if (command === 'get_mouse_position') {
        return [240, 320];
      }
      if (command === 'start_microphone_capture') {
        throw new Error('Microphone is unavailable.');
      }
      if (command === 'inject_text' || command === 'stop_microphone_capture') {
        return undefined;
      }
      return undefined;
    });

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        warmup: 'error',
        lastErrorSource: 'microphone',
        lastErrorMessage: 'Microphone is unavailable.',
      })
    );
  });

  it('anchors the overlay to the text cursor and publishes preparing then listening before capture stays warm', async () => {
    const callOrder: string[] = [];
    mocks.mockStart.mockImplementation(async () => {
      callOrder.push('start');
    });
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_text_cursor_position') {
        callOrder.push('get_text_cursor_position');
        return [120, 280];
      }
      if (command === 'start_microphone_capture') {
        callOrder.push('start_microphone_capture');
        return undefined;
      }
      return undefined;
    });

    const service = await loadService();
    await service.startListening();

    expect(mocks.windowOpen).toHaveBeenCalledWith(116, 288);
    expect(callOrder.indexOf('start_microphone_capture')).toBeGreaterThan(
      callOrder.indexOf('start')
    );
    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        sessionId: 'voice-typing-1',
        phase: 'preparing',
        text: '',
        revision: 1,
      })
    );
    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        sessionId: 'voice-typing-1',
        phase: 'listening',
        text: '',
        revision: 2,
      })
    );
  });

  it('does not let a late listening placeholder overwrite an already visible segment', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    let resolveStart: (() => void) | undefined;
    mocks.mockStart.mockImplementation((segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
      return new Promise<void>((resolve) => {
        resolveStart = resolve;
      });
    });

    const service = await loadService();
    const startPromise = service.startListening();
    await flushMicrotasks(2);

    onSegment?.({ id: 'seg-early', text: '提早预览', isFinal: false });
    await flushMicrotasks();
    resolveStart?.();
    await startPromise;

    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
      'preparing',
      'segment',
    ]);
  });

  it('keeps an existing candidate visible when later partials collapse to empty text', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '测试123', isFinal: false });
    await flushMicrotasks(8);
    onSegment?.({ id: 'seg-1', text: '   ', isFinal: false });
    await flushMicrotasks(8);

    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual(['segment']);
    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.text)).toEqual(['测试 123']);
    expect(mocks.loggerInfo).toHaveBeenCalledWith(
      '[VoiceTypingSessionMachine] Dropped segment update',
      expect.objectContaining({
        dropReason: 'empty_after_normalize',
      })
    );
  });

  it('allows punctuation-only partial updates to appear as candidate text', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '。', isFinal: false });
    await flushMicrotasks(8);

    expect(mocks.windowSendState).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'segment',
        text: '。',
        segmentId: 'seg-1',
        isFinal: false,
      })
    );
  });

  it('keeps publishing later partial updates for the same sentence instead of only the first draft', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '喂。', isFinal: false });
    await flushMicrotasks(8);
    onSegment?.({ id: 'seg-1', text: '喂，123。', isFinal: false });
    await flushMicrotasks(8);
    onSegment?.({ id: 'seg-1', text: '喂，123，继续。', isFinal: false });
    await flushMicrotasks(8);

    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
      'segment',
      'segment',
      'segment',
    ]);
    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.text)).toEqual([
      '喂。',
      '喂，123。',
      '喂，123，继续。',
    ]);
    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.revision)).toEqual([
      3, 4, 5,
    ]);
  });

  it('drops non-final partial updates after manual stop is requested', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    let resolveSoftStop: (() => void) | undefined;
    vi.useFakeTimers();
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });
    mocks.mockSoftStop.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveSoftStop = resolve;
        })
    );

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    const stopPromise = service.stopListening();
    await flushMicrotasks(2);

    onSegment?.({ id: 'seg-1', text: '停止后的草稿', isFinal: false });
    await flushMicrotasks(8);

    expect(mocks.windowSendState).not.toHaveBeenCalled();
    expect(mocks.loggerInfo).toHaveBeenCalledWith(
      '[VoiceTypingSessionMachine] Dropped segment update',
      expect.objectContaining({
        dropReason: 'manual_stop_pending',
        segmentId: 'seg-1',
      })
    );

    resolveSoftStop?.();
    await vi.runAllTimersAsync();
    await stopPromise;
  });

  it('commits a finalized sentence and repositions the candidate bar from the updated caret', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    let cursorCallCount = 0;
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_text_cursor_position') {
        cursorCallCount += 1;
        return cursorCallCount === 1 ? [120, 280] : [160, 340];
      }
      if (command === 'start_microphone_capture' || command === 'inject_text') {
        return undefined;
      }
      if (command === 'get_mouse_position') {
        return [240, 320];
      }
      return undefined;
    });
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '你好世', isFinal: false });
    await flushMicrotasks(8);
    onSegment?.({ id: 'seg-1', text: '你好世界', isFinal: true });
    await flushMicrotasks(20);

    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        sessionId: 'voice-typing-1',
        phase: 'segment',
        text: '你好世',
        segmentId: 'seg-1',
        isFinal: false,
        revision: 3,
      })
    );
    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        sessionId: 'voice-typing-1',
        phase: 'segment',
        text: '你好世界',
        segmentId: 'seg-1',
        isFinal: true,
        revision: 4,
      })
    );
    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({
        sessionId: 'voice-typing-1',
        phase: 'listening',
        text: '',
        revision: 5,
      })
    );
    expect(getInvokeCalls('inject_text')).toEqual([
      ['inject_text', { text: '你好世界', shortcutModifiers: ['alt'] }],
    ]);
    expect(mocks.windowOpen).toHaveBeenCalledWith(156, 348);
  });

  it('retries caret lookup after commit until the cursor position actually moves', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    let cursorCallCount = 0;
    vi.useFakeTimers();
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_text_cursor_position') {
        cursorCallCount += 1;
        if (cursorCallCount <= 3) {
          return [120, 280];
        }
        return [180, 360];
      }
      if (command === 'start_microphone_capture' || command === 'inject_text') {
        return undefined;
      }
      if (command === 'get_mouse_position') {
        return [240, 320];
      }
      return undefined;
    });
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });
    await flushMicrotasks(8);
    await vi.advanceTimersByTimeAsync(39);
    expect(mocks.windowOpen).not.toHaveBeenCalledWith(176, 368);
    await vi.advanceTimersByTimeAsync(1);
    await flushMicrotasks(8);
    await vi.advanceTimersByTimeAsync(40);
    await flushMicrotasks(8);

    expect(getInvokeCalls('inject_text')).toEqual([
      ['inject_text', { text: '第一句', shortcutModifiers: ['alt'] }],
    ]);
    expect(mocks.windowOpen).toHaveBeenCalledWith(176, 368);
  });

  it('passes control modifiers to inject_text when the voice typing shortcut uses Ctrl + Space', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingShortcut: 'Ctrl + Space',
    };
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-ctrl', text: '123。', isFinal: true });
    await flushMicrotasks(12);

    expect(getInvokeCalls('inject_text')).toEqual([
      ['inject_text', { text: '123。', shortcutModifiers: ['control'] }],
    ]);
  });

  it('omits shortcut modifiers when the voice typing shortcut has no modifier keys', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingShortcut: 'Space',
    };
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-space', text: '纯文本', isFinal: true });
    await flushMicrotasks(12);

    expect(getInvokeCalls('inject_text')).toEqual([['inject_text', { text: '纯文本' }]]);
  });

  it('keeps the session alive across VAD sentence boundaries in hold mode', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    vi.useFakeTimers();
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });
    await flushMicrotasks(8);
    await vi.advanceTimersByTimeAsync(120);
    await flushMicrotasks(8);
    onSegment?.({ id: 'seg-2', text: '第二句草稿', isFinal: false });
    await flushMicrotasks(8);

    expect(getInvokeCalls('inject_text')).toEqual([
      ['inject_text', { text: '第一句', shortcutModifiers: ['alt'] }],
    ]);
    expect(mocks.windowClose).not.toHaveBeenCalled();
    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.phase)).toEqual([
      'segment',
      'listening',
      'segment',
    ]);
    expect(mocks.windowSendState.mock.calls.map(([payload]) => payload.text)).toEqual([
      '第一句',
      '',
      '第二句草稿',
    ]);
  });

  it('keeps overlay revisions monotonic after a session is stopped and restarted', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    vi.useFakeTimers();
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    onSegment?.({ id: 'seg-1', text: '第一轮候选', isFinal: false });
    await flushMicrotasks(8);

    const firstStopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await firstStopPromise;

    vi.clearAllMocks();

    await service.startListening();
    onSegment?.({ id: 'seg-2', text: '第二轮候选', isFinal: false });
    await flushMicrotasks(8);

    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        sessionId: 'voice-typing-2',
        phase: 'preparing',
        revision: 4,
      })
    );
    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        sessionId: 'voice-typing-2',
        phase: 'listening',
        revision: 5,
      })
    );
    expect(mocks.windowSendState).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({
        sessionId: 'voice-typing-2',
        phase: 'segment',
        text: '第二轮候选',
        revision: 6,
      })
    );
  });

  it('uses pressed events to toggle start and stop in toggle mode', async () => {
    vi.useFakeTimers();
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingMode: 'toggle',
    };

    const service = await loadService();
    service.init();
    await flushMicrotasks();

    expect(mocks.register).toHaveBeenCalled();
    expect(mocks.shortcutState.handler).toBeTruthy();

    mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Pressed' });
    await flushMicrotasks();

    expect(mocks.mockStart).toHaveBeenCalledTimes(1);

    mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Released' });
    await flushMicrotasks();

    expect(mocks.mockSoftStop).not.toHaveBeenCalled();

    vi.advanceTimersByTime(500);
    mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Pressed' });
    await vi.runAllTimersAsync();
    expect(mocks.mockSoftStop).toHaveBeenCalledTimes(1);
  });

  it('closes only after a flush-only final sentence is committed during manual stop', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    let resolveSoftStop: (() => void) | undefined;
    vi.useFakeTimers();
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });
    mocks.mockSoftStop.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveSoftStop = resolve;
        })
    );

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    const stopPromise = service.stopListening();
    await flushMicrotasks(2);

    expect(mocks.windowClose).not.toHaveBeenCalled();

    onSegment?.({ id: 'seg-1', text: '短句结果', isFinal: true });
    await flushMicrotasks(8);

    expect(getInvokeCalls('inject_text')).toEqual([
      ['inject_text', { text: '短句结果', shortcutModifiers: ['alt'] }],
    ]);
    expect(mocks.windowSendState).toHaveBeenCalledTimes(1);
    expect(mocks.windowSendState).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'segment',
        text: '短句结果',
        segmentId: 'seg-1',
        isFinal: true,
        revision: 3,
      })
    );
    expect(mocks.windowClose).not.toHaveBeenCalled();

    resolveSoftStop?.();
    await vi.advanceTimersByTimeAsync(79);
    expect(mocks.windowClose).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await stopPromise;

    expect(mocks.windowClose).toHaveBeenCalledTimes(1);
    expect(mocks.windowClearState).toHaveBeenCalledTimes(1);
  });

  it('ignores duplicate finals for a sentence that was already committed', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    vi.useFakeTimers();
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '重复句子', isFinal: true });
    await flushMicrotasks(8);
    await vi.advanceTimersByTimeAsync(120);
    await flushMicrotasks(8);
    onSegment?.({ id: 'seg-1', text: '重复句子', isFinal: true });
    await flushMicrotasks(8);

    expect(getInvokeCalls('inject_text')).toEqual([
      ['inject_text', { text: '重复句子', shortcutModifiers: ['alt'] }],
    ]);
    const phases = mocks.windowSendState.mock.calls.map(([payload]) => payload.phase);
    expect(phases.filter((phase) => phase === 'segment')).toEqual(['segment']);
    expect(phases).toContain('listening');
  });

  it('ignores stale segment updates after the session is closed', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    vi.useFakeTimers();
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-stale', text: '过期结果', isFinal: false });
    await flushMicrotasks();

    expect(mocks.windowSendState).not.toHaveBeenCalled();
    expect(mocks.windowOpen).not.toHaveBeenCalled();
  });

  it('flushes the recognizer before closing the voice typing session', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      keepMicrophoneActive: true,
    };
    vi.useFakeTimers();
    const service = await loadService();

    await service.startListening();
    vi.clearAllMocks();

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;

    expect(mocks.invoke).not.toHaveBeenCalledWith('stop_microphone_capture', expect.anything());
    expect(mocks.mockSoftStop).toHaveBeenCalled();
    expect(mocks.windowClose).toHaveBeenCalled();
  });

  it('propagates session errors into the runtime status store', async () => {
    mocks.mockStart.mockImplementation(
      async (_segmentCallback: (segment: any) => void, errorCallback: (error: string) => void) => {
        errorCallback('Recognizer crashed.');
      }
    );

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    await service.startListening();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        lastErrorSource: 'session',
        lastErrorMessage: 'Recognizer crashed.',
      })
    );
  });
  it('clears previous session failure when startListening is called again', async () => {
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();
    useVoiceTypingRuntimeStore
      .getState()
      .reportRuntimeError('session', 'Live transcription consumer voice-typing is already active');
    expect(useVoiceTypingRuntimeStore.getState().lastErrorSource).toBe('session');

    const service = await loadService();
    await service.startListening();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState().lastErrorSource).toBeNull();
    expect(useVoiceTypingRuntimeStore.getState().lastErrorMessage).toBeNull();
  });

  it('clears stale shortcut errors after the shortcut changes and registration recovers', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };
    mocks.register.mockRejectedValueOnce(new Error('Shortcut conflict.'));

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'error',
        lastErrorSource: 'shortcut_registration',
      })
    );

    emitConfigChange({ voiceTypingShortcut: 'Ctrl + Shift + V' });
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'ready',
        warmup: 'ready',
        lastErrorSource: null,
        lastErrorMessage: null,
      })
    );
  });

  it('clears stale warm-up errors after a dependency change triggers recovery', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };
    mocks.mockPrepare.mockRejectedValueOnce(new Error('Warm-up failed once.'));

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        warmup: 'error',
        lastErrorSource: 'warmup',
      })
    );

    emitConfigChange({ microphoneId: 'usb-mic' });
    await flushMicrotasks(8);

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'ready',
        warmup: 'ready',
        lastErrorSource: null,
        lastErrorMessage: null,
      })
    );
  });

  it('retries warm-up explicitly for diagnostics and restores runtime readiness', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      keepMicrophoneActive: true,
    };

    const service = await loadService();
    const { useVoiceTypingRuntimeStore } = await loadRuntimeStore();

    service.init();
    await flushMicrotasks(8);

    useVoiceTypingRuntimeStore.getState().reportRuntimeError('warmup', 'Warm-up failed once.');
    vi.clearAllMocks();

    await service.retryWarmup();
    await flushMicrotasks(8);

    expect(getInvokeCalls('stop_microphone_capture')).toEqual([
      ['stop_microphone_capture', { instanceId: 'voice-typing' }],
    ]);
    expect(getInvokeCalls('start_microphone_capture')).toEqual([
      [
        'start_microphone_capture',
        {
          deviceName: null,
          instanceId: 'voice-typing',
        },
      ],
    ]);
    expect(mocks.mockPrepare).toHaveBeenCalled();
    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'ready',
        warmup: 'ready',
        lastErrorSource: null,
        lastErrorMessage: null,
      })
    );
  });

  it('cancels startup cleanly if the shortcut is released before microphone capture begins', async () => {
    let resolveStart: (() => void) | undefined;
    vi.useFakeTimers();
    mocks.mockStart.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveStart = resolve;
        })
    );

    const service = await loadService();
    const startPromise = service.startListening();
    await flushMicrotasks(2);

    const stopPromise = service.stopListening();
    resolveStart?.();
    await startPromise;
    await vi.runAllTimersAsync();
    await stopPromise;

    expect(mocks.invoke).not.toHaveBeenCalledWith('start_microphone_capture', expect.anything());
    expect(mocks.mockSoftStop).toHaveBeenCalled();
  });
  it('cancels an active session cleanly without injecting text', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-cancel', text: '这句将被取消', isFinal: false });
    await flushMicrotasks(8);

    await service.cancelListening();
    await flushMicrotasks(8);

    // Verify softStop was called
    expect(mocks.mockSoftStop).toHaveBeenCalled();
    // Verify inject_text was NEVER called
    expect(getInvokeCalls('inject_text')).toEqual([]);
    // Verify window was closed
    expect(mocks.windowClose).toHaveBeenCalled();
  });
  it('polishes accumulated speech in polish mode before injection', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingProcessingMode: 'polish',
    };
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '那个就是说今天天气不错', isFinal: true });
    await flushMicrotasks(8);

    // In polish mode, isFinal segments shouldn't immediately inject
    expect(getInvokeCalls('inject_text')).toEqual([]);

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;
    await flushMicrotasks(8);

    // After stopping, polish was called and injected
    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls.length).toBe(1);
    expect(injectCalls[0][1].text).toContain('[polished]');
  });
  it('automatically commits and translates on prolonged VAD silence in translation mode', async () => {
    let onSegment: ((segment: { id: string; text: string; isFinal: boolean }) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingTargetLanguage: 'en',
    };
    mocks.mockStart.mockImplementation(
      async (
        segmentCallback: (segment: { id: string; text: string; isFinal: boolean }) => void
      ) => {
        onSegment = segmentCallback;
      }
    );

    const service = await loadService();
    await service.startListening({ isTranslate: true });
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '你好世界', isFinal: true });
    await flushMicrotasks(8);

    // Immediately after segment, no injection has happened yet
    expect(getInvokeCalls('inject_text')).toEqual([]);

    // Advance timer by 1000ms (< 1500ms default silence timeout)
    await vi.advanceTimersByTimeAsync(1000);
    await flushMicrotasks(8);
    expect(getInvokeCalls('inject_text')).toEqual([]);

    // Advance timer to trigger auto-commit (remaining 500ms + flush)
    await vi.advanceTimersByTimeAsync(600);
    await flushMicrotasks(16);

    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls.length).toBe(1);
    expect(injectCalls[0][1].text).toContain('[translated to en] 你好世界');
  });

  it('cancels the silence timer and continues accumulating when speech resumes in translation mode', async () => {
    let onSegment: ((segment: { id: string; text: string; isFinal: boolean }) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingTargetLanguage: 'en',
    };
    mocks.mockStart.mockImplementation(
      async (
        segmentCallback: (segment: { id: string; text: string; isFinal: boolean }) => void
      ) => {
        onSegment = segmentCallback;
      }
    );

    const service = await loadService();
    await service.startListening({ isTranslate: true });
    vi.clearAllMocks();

    // Utterance 1 finishes
    onSegment?.({ id: 'seg-1', text: '第一句', isFinal: true });
    await flushMicrotasks(8);
    expect(getInvokeCalls('inject_text')).toEqual([]);

    // 800ms later, user starts speaking utterance 2 (interim speech)
    await vi.advanceTimersByTimeAsync(800);
    onSegment?.({ id: 'seg-2', text: '第二句', isFinal: false });
    await flushMicrotasks(8);

    // Another 1000ms passes (total 1800ms since seg-1, but timer was reset by seg-2 interim)
    await vi.advanceTimersByTimeAsync(1000);
    await flushMicrotasks(8);
    expect(getInvokeCalls('inject_text')).toEqual([]);

    // Utterance 2 finishes
    onSegment?.({ id: 'seg-2', text: '第二句', isFinal: true });
    await flushMicrotasks(8);
    expect(getInvokeCalls('inject_text')).toEqual([]);

    // Now silence elapses for 1500ms + 100ms flush settle
    await vi.advanceTimersByTimeAsync(1600);
    await flushMicrotasks(16);
    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls.length).toBe(1);
    expect(injectCalls[0][1].text).toContain('[translated to en] 第一句第二句');
  });

  it('automatically commits on prolonged VAD silence in polish mode', async () => {
    let onSegment: ((segment: { id: string; text: string; isFinal: boolean }) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingProcessingMode: 'polish',
    };
    mocks.mockStart.mockImplementation(
      async (
        segmentCallback: (segment: { id: string; text: string; isFinal: boolean }) => void
      ) => {
        onSegment = segmentCallback;
      }
    );

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '自动润色测试句子', isFinal: true });
    await flushMicrotasks(8);
    expect(getInvokeCalls('inject_text')).toEqual([]);

    // Advance timer past silence timeout + flush settle
    await vi.advanceTimersByTimeAsync(1600);
    await flushMicrotasks(16);
    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls.length).toBe(1);
    expect(injectCalls[0][1].text).toContain('[polished] 自动润色测试句子');
  });
  it('positions overlay at bottom center when voiceTypingPlacement is bottom_center', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingPlacement: 'bottom_center',
    };

    const service = await loadService();
    await service.startListening();
    await flushMicrotasks(4);

    expect(mocks.windowPrepare).toHaveBeenCalledWith([760, 992]);
  });

  it('positions overlay at true physical center on monitors with DPI scaleFactor > 1', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingPlacement: 'bottom_center',
    };
    mocks.monitorFromPoint.mockResolvedValueOnce({
      scaleFactor: 1.5,
      workArea: {
        position: { x: 0, y: 0 },
        size: { width: 2560, height: 1400 },
      },
      position: { x: 0, y: 0 },
      size: { width: 2560, height: 1400 },
    });

    const service = await loadService();
    await service.startListening();
    await flushMicrotasks(4);

    expect(mocks.windowPrepare).toHaveBeenCalledWith([980, 1268]);
  });
  it('transforms selection context when focused selection is present', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
    };
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_focused_selection_text') {
        return '原始选中的文本';
      }
      if (command === 'get_mouse_position') {
        return [240, 320];
      }
      return undefined;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '翻译成日文', isFinal: true });
    await flushMicrotasks(8);

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;
    await flushMicrotasks(8);

    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls.length).toBe(1);
    expect(injectCalls[0][1].text).toContain('[transformed: 原始选中的文本]');
  });

  it('applies text replacements and dynamic macros before injecting', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      textReplacementSets: [
        {
          id: 'set-1',
          name: 'Snippets',
          enabled: true,
          ignoreCase: true,
          rules: [{ id: 'r1', from: '我的邮箱', to: 'asoda@outlook.com' }],
        },
      ],
    };
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });

    const service = await loadService();
    await service.startListening();
    vi.clearAllMocks();

    onSegment?.({ id: 'seg-1', text: '请发送到 我的邮箱 谢谢', isFinal: true });
    await flushMicrotasks(8);

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;
    await flushMicrotasks(8);

    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls.length).toBe(1);
    expect(injectCalls[0][1].text).toContain('asoda@outlook.com');
  });
  it('opens quick recall drawer overlay with focus enabled and passes history in payload', async () => {
    const { useVoiceTypingHistoryStore } = await import('../../stores/voiceTypingHistoryStore');
    useVoiceTypingHistoryStore.getState().clearHistory();
    useVoiceTypingHistoryStore.getState().addItem({
      rawText: 'Test history',
      injectedText: 'Test history',
      mode: 'raw',
    });

    const service = await loadService();
    service.init();
    vi.clearAllMocks();

    await service.openQuickRecall();

    expect(mocks.windowSendState).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'recall',
        history: expect.arrayContaining([
          expect.objectContaining({
            rawText: 'Test history',
            injectedText: 'Test history',
          }),
        ]),
      })
    );
    expect(mocks.windowOpen).toHaveBeenCalledWith(expect.any(Number), expect.any(Number), true);
  });

  it('reinjects text from quick recall with focus settle delay', async () => {
    const service = await loadService();
    service.init();
    await flushMicrotasks(2);

    const reinjectHandler = mocks.eventListeners['voice-typing:reinject'];
    expect(reinjectHandler).toBeDefined();

    const reinjectPromise = reinjectHandler({ payload: { text: 'History input content' } });
    await flushMicrotasks(2);

    // Before 80ms delay completes, inject_text has not been called yet
    expect(getInvokeCalls('inject_text')).toHaveLength(0);

    // Advance past the 80ms focus settle delay
    await vi.advanceTimersByTimeAsync(80);
    await reinjectPromise;

    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls).toHaveLength(1);
    expect(injectCalls[0][1].text).toBe('History input content');
  });

  it('positions quick recall higher up in bottom_center mode to avoid bottom screen overflow', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingPlacement: 'bottom_center',
    };

    const service = await loadService();
    service.init();
    await flushMicrotasks(4);
    vi.clearAllMocks();

    await service.openQuickRecall();

    // workHeight = 1080, scale = 1.
    // windowPhysicalBottomMargin = (48 + 280) * 1 = 328.
    // targetY = 1080 - 328 = 752.
    expect(mocks.windowPrepare).toHaveBeenCalledWith([760, 752]);
  });

  it('flips quick recall above cursor if cursor is near bottom of screen in caret mode', async () => {
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingPlacement: 'caret',
    };

    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_text_cursor_position') {
        // Cursor is at y = 950 (near bottom of 1040 workHeight screen)
        return [500, 950];
      }
      return undefined;
    });

    const service = await loadService();
    service.init();
    await flushMicrotasks(4);
    vi.clearAllMocks();

    await service.openQuickRecall();

    // With cursor at 950, 950 + 280 = 1230 > maxBottom (1080 - 16 = 1064)
    expect(mocks.windowPrepare).toHaveBeenCalledWith([492, 666]);
  });

  it('senses application context and adapts overlay mode and polish context', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingProcessingMode: 'polish',
      voiceTypingContextAwarenessEnabled: true,
    };
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_foreground_window_info') {
        return { appName: 'code.exe', windowTitle: 'editor.ts - Project' };
      }
      if (command === 'get_mouse_position') {
        return [240, 320];
      }
      if (command === 'get_text_cursor_position') {
        return [120, 280];
      }
      return undefined;
    });

    const service = await loadService();
    await service.startListening();
    await flushMicrotasks(4);

    expect(mocks.windowSendState).toHaveBeenCalledWith(
      expect.objectContaining({
        contextMode: 'developer',
      })
    );

    onSegment?.({ id: 'seg-1', text: 'define a new variable', isFinal: true });
    await flushMicrotasks(4);

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;
    await flushMicrotasks(8);

    expect(mocks.polishVoiceTypingText).toHaveBeenCalledWith(
      'define a new variable',
      expect.objectContaining({
        context: expect.objectContaining({
          appName: 'code.exe',
          mode: 'developer',
        }),
      })
    );
  });

  it('strips trailing full stops in chat context mode during raw dictation', async () => {
    let onSegment: ((segment: any) => void) | undefined;
    mocks.config = {
      ...mocks.defaultConfig,
      voiceTypingEnabled: true,
      voiceTypingProcessingMode: 'raw',
      voiceTypingContextAwarenessEnabled: true,
    };
    mocks.mockStart.mockImplementation(async (segmentCallback: (segment: any) => void) => {
      onSegment = segmentCallback;
    });
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_foreground_window_info') {
        return { appName: 'slack.exe', windowTitle: 'General - Sona' };
      }
      if (command === 'get_mouse_position') {
        return [240, 320];
      }
      if (command === 'get_text_cursor_position') {
        return [120, 280];
      }
      return undefined;
    });

    const service = await loadService();
    await service.startListening();
    await flushMicrotasks(4);

    expect(mocks.windowSendState).toHaveBeenCalledWith(
      expect.objectContaining({
        contextMode: 'chat',
      })
    );

    onSegment?.({ id: 'seg-1', text: 'see you tomorrow.', isFinal: true });
    await flushMicrotasks(4);

    const stopPromise = service.stopListening();
    await vi.runAllTimersAsync();
    await stopPromise;
    await flushMicrotasks(8);

    const injectCalls = getInvokeCalls('inject_text');
    expect(injectCalls.length).toBe(1);
    expect(injectCalls[0][1].text).toBe('see you tomorrow');
  });
});
