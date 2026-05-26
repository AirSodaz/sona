import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
  const shortcutState: { handler?: (event: any) => void } = {};

  return {
    shortcutState,
    register: vi.fn(async (_shortcut: string, handler: (event: any) => void) => {
      shortcutState.handler = handler;
    }),
    unregister: vi.fn(async () => undefined),
    isRegistered: vi.fn(async () => true),
    loggerInfo: vi.fn(),
    loggerError: vi.fn(),
  };
});

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
  register: mocks.register,
  unregister: mocks.unregister,
  isRegistered: mocks.isRegistered,
}));

vi.mock('../../../utils/logger', () => ({
  logger: {
    info: mocks.loggerInfo,
    error: mocks.loggerError,
  },
}));

async function loadController() {
  const controllerModule = await import('../voiceTypingShortcutController');
  const storeModule = await import('../../../stores/voiceTypingRuntimeStore');
  storeModule.useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
  return {
    VoiceTypingShortcutController: controllerModule.VoiceTypingShortcutController,
    useVoiceTypingRuntimeStore: storeModule.useVoiceTypingRuntimeStore,
  };
}

describe('VoiceTypingShortcutController', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mocks.shortcutState.handler = undefined;
    mocks.isRegistered.mockResolvedValue(true);
  });

  it('normalizes and registers an enabled shortcut', async () => {
    const { VoiceTypingShortcutController, useVoiceTypingRuntimeStore } = await loadController();
    const controller = new VoiceTypingShortcutController({
      getMode: () => 'hold',
      isListening: () => false,
      startListening: vi.fn(),
      stopListening: vi.fn(),
    });

    await controller.update(true, 'Ctrl + Shift + V');

    expect(mocks.register).toHaveBeenCalledWith('Ctrl+Shift+V', expect.any(Function));
    expect(useVoiceTypingRuntimeStore.getState().shortcutRegistration).toBe('ready');
  });

  it('unregisters the previous shortcut before registering the new one', async () => {
    const { VoiceTypingShortcutController } = await loadController();
    const controller = new VoiceTypingShortcutController({
      getMode: () => 'hold',
      isListening: () => false,
      startListening: vi.fn(),
      stopListening: vi.fn(),
    });

    await controller.update(true, 'Alt+V');
    await controller.update(true, 'Ctrl+Space');

    expect(mocks.isRegistered).toHaveBeenCalledWith('Alt+V');
    expect(mocks.unregister).toHaveBeenCalledWith('Alt+V');
    expect(mocks.register).toHaveBeenLastCalledWith('Ctrl+Space', expect.any(Function));
  });

  it('dispatches hold mode press and release to start and stop listening', async () => {
    const startListening = vi.fn(async () => undefined);
    const stopListening = vi.fn(async () => undefined);
    let active = false;
    const { VoiceTypingShortcutController } = await loadController();
    const controller = new VoiceTypingShortcutController({
      getMode: () => 'hold',
      isListening: () => active,
      startListening: async () => {
        active = true;
        await startListening();
      },
      stopListening: async () => {
        active = false;
        await stopListening();
      },
    });

    await controller.update(true, 'Alt+V');
    mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Pressed' });
    await Promise.resolve();
    mocks.shortcutState.handler?.({ shortcut: 'Alt+V', state: 'Released' });
    await Promise.resolve();

    expect(startListening).toHaveBeenCalledTimes(1);
    expect(stopListening).toHaveBeenCalledTimes(1);
  });

  it('reports registration failures to runtime status', async () => {
    mocks.register.mockRejectedValueOnce(new Error('Shortcut already registered.'));
    const { VoiceTypingShortcutController, useVoiceTypingRuntimeStore } = await loadController();
    const controller = new VoiceTypingShortcutController({
      getMode: () => 'hold',
      isListening: () => false,
      startListening: vi.fn(),
      stopListening: vi.fn(),
    });

    await controller.update(true, 'Alt+V');

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        shortcutRegistration: 'error',
        lastErrorSource: 'shortcut_registration',
        lastErrorMessage: 'Shortcut already registered.',
      }),
    );
  });
});
