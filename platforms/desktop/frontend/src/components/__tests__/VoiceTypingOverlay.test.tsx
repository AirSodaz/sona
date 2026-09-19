import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useVoiceTypingHistoryStore } from '../../stores/voiceTypingHistoryStore';
import { VoiceTypingOverlay } from '../VoiceTypingOverlay';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, opts?: any) => {
        if (key === 'common.listening') return '正在聆听...';
        if (key === 'common.preparing') return '正在准备...';
        if (opts?.defaultValue) return opts.defaultValue;
        return key;
      },
    }),
  };
});

const mocks = vi.hoisted(() => {
  const listenCallbacks: Record<string, (event: any) => void> = {};
  let storeKeyChangeCallback: ((value: any) => void) | null = null;
  const unlisten = vi.fn();
  const emit = vi.fn().mockResolvedValue(undefined);
  const listen = vi.fn((event: string, callback: (event: any) => void) => {
    listenCallbacks[event] = callback;
    return Promise.resolve(() => {
      delete listenCallbacks[event];
      unlisten();
    });
  });
  const invoke = vi.fn(async (command: string): Promise<any> => {
    if (command === 'get_aux_window_state') {
      return null;
    }
    return undefined;
  });
  const settingsGet = vi.fn().mockResolvedValue(null);
  const settingsOnKeyChange = vi.fn((_key: string, callback: (value: any) => void) => {
    storeKeyChangeCallback = callback;
    return Promise.resolve(() => {
      if (storeKeyChangeCallback === callback) {
        storeKeyChangeCallback = null;
      }
    });
  });
  const currentWindowScaleFactor = vi.fn().mockResolvedValue(1);
  const currentWindowInnerSize = vi.fn().mockResolvedValue({ width: 400, height: 80 });
  const currentWindowSetSize = vi.fn().mockResolvedValue(undefined);
  const currentWindowInnerPosition = vi.fn().mockResolvedValue({ x: 100, y: 100 });
  const currentWindowSetPosition = vi.fn().mockResolvedValue(undefined);
  const currentWindowSetFocus = vi.fn().mockResolvedValue(undefined);
  const currentMonitor = vi.fn().mockResolvedValue({
    scaleFactor: 1,
    position: { x: 0, y: 0 },
    size: { width: 1920, height: 1080 },
    workArea: {
      position: { x: 0, y: 0 },
      size: { width: 1920, height: 1040 },
    },
  });

  return {
    currentWindowInnerSize,
    currentWindowScaleFactor,
    currentWindowSetSize,
    currentWindowInnerPosition,
    currentWindowSetPosition,
    currentWindowSetFocus,
    currentMonitor,
    invoke,
    emit,
    listen,
    listenCallbacks,
    currentWindowListen: vi.fn((event: string, callback: (event: any) => void) => {
      listenCallbacks[`window:${event}`] = callback;
      return Promise.resolve(() => {
        delete listenCallbacks[`window:${event}`];
        unlisten();
      });
    }),
    unlisten,
    loggerInfo: vi.fn(),
    loggerWarn: vi.fn(),
    loggerError: vi.fn(),
    loggerDebug: vi.fn(),
    settingsGet,
    settingsOnKeyChange,
    triggerStoredConfigChange: (value: any) => {
      storeKeyChangeCallback?.(value);
    },
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.listen,
  emit: mocks.emit,
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    listen: mocks.currentWindowListen,
  }),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    scaleFactor: mocks.currentWindowScaleFactor,
    innerSize: mocks.currentWindowInnerSize,
    setSize: mocks.currentWindowSetSize,
    innerPosition: mocks.currentWindowInnerPosition,
    setPosition: mocks.currentWindowSetPosition,
    setFocus: mocks.currentWindowSetFocus,
  }),
  currentMonitor: () => mocks.currentMonitor(),
}));

vi.mock('@tauri-apps/api/dpi', () => ({
  PhysicalSize: class MockPhysicalSize {
    constructor(
      public width: number,
      public height: number
    ) {}
  },
  PhysicalPosition: class MockPhysicalPosition {
    constructor(
      public x: number,
      public y: number
    ) {}
  },
}));

vi.mock('../../services/storageService', () => ({
  STORE_KEY_CONFIG: 'sona-config',
  settingsStore: {
    get: mocks.settingsGet,
    onKeyChange: mocks.settingsOnKeyChange,
  },
}));

vi.mock('../../utils/logger', () => ({
  logger: {
    info: mocks.loggerInfo,
    warn: mocks.loggerWarn,
    error: mocks.loggerError,
    debug: mocks.loggerDebug,
  },
}));

describe('VoiceTypingOverlay', () => {
  let mediaQueryMatches = false;
  let mediaQueryListeners: Array<(event: MediaQueryListEvent) => void> = [];
  let resizeObserverCallback: ResizeObserverCallback | null = null;

  beforeEach(() => {
    vi.useRealTimers();
    for (const key of Object.keys(mocks.listenCallbacks)) {
      delete mocks.listenCallbacks[key];
    }
    mocks.invoke.mockImplementation(async (command: string): Promise<any> => {
      if (command === 'get_aux_window_state') {
        return null;
      }
      return undefined;
    });
    mocks.settingsGet.mockResolvedValue(null);
    mocks.settingsOnKeyChange.mockClear();
    mocks.currentWindowScaleFactor.mockResolvedValue(1);
    mocks.currentWindowInnerSize.mockResolvedValue({ width: 400, height: 80 });
    mocks.currentWindowSetSize.mockResolvedValue(undefined);
    document.documentElement.style.background = '';
    document.body.style.background = '';
    document.documentElement.removeAttribute('data-theme');

    mediaQueryMatches = false;
    mediaQueryListeners = [];
    resizeObserverCallback = null;

    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation(() => ({
        matches: mediaQueryMatches,
        media: '(prefers-color-scheme: dark)',
        addEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => {
          mediaQueryListeners.push(listener);
        },
        removeEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => {
          mediaQueryListeners = mediaQueryListeners.filter(
            (currentListener) => currentListener !== listener
          );
        },
      })),
    });

    globalThis.ResizeObserver = class {
      constructor(callback: ResizeObserverCallback) {
        resizeObserverCallback = callback;
      }

      observe() {}

      disconnect() {}

      unobserve() {}
    } as typeof ResizeObserver;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows the listening placeholder by default and sets a transparent background', () => {
    render(<VoiceTypingOverlay />);

    screen.getByText('正在聆听...');
    expect(document.documentElement.style.background).toBe('transparent');
    expect(document.body.style.background).toBe('transparent');
  });

  it('applies the stored dark theme to the auxiliary window root', async () => {
    mocks.settingsGet.mockResolvedValue({
      theme: 'dark',
    });

    render(<VoiceTypingOverlay />);

    await waitFor(() => {
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    });
  });

  it('resolves auto theme and reacts to system color-scheme changes', async () => {
    mocks.settingsGet.mockResolvedValue({
      theme: 'auto',
    });

    render(<VoiceTypingOverlay />);

    await waitFor(() => {
      expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    });

    await act(async () => {
      mediaQueryMatches = true;
      for (const listener of mediaQueryListeners) {
        listener({ matches: true } as MediaQueryListEvent);
      }
    });

    await waitFor(() => {
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    });
  });

  it('updates the resolved theme when the stored config changes', async () => {
    mocks.settingsGet.mockResolvedValue({
      theme: 'light',
    });

    render(<VoiceTypingOverlay />);

    await waitFor(() => {
      expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    });

    await act(async () => {
      mocks.triggerStoredConfigChange({ theme: 'dark' });
    });

    await waitFor(() => {
      expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    });
  });

  it('renders the latest preview text from overlay events', async () => {
    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'voice-typing-1',
          text: 'Transcription preview test',
          phase: 'segment',
          segmentId: 'seg-1',
          isFinal: false,
          revision: 1,
        },
      });
    });

    screen.getByText('Transcription preview test');
    expect(screen.getByTestId('voice-typing-bubble').style.background).toBe(
      'var(--color-bg-elevated)'
    );
  });

  it('renders punctuation-only segment text and records the rendered phase', async () => {
    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'voice-typing-2',
          text: '。',
          phase: 'segment',
          segmentId: 'seg-2',
          isFinal: false,
          revision: 2,
        },
      });
    });

    screen.getByText('。');
    expect(mocks.loggerInfo).toHaveBeenCalledWith(
      '[VoiceTypingOverlay] Rendered segment state',
      expect.objectContaining({
        renderedPhase: 'segment',
        textLength: 1,
      })
    );
  });

  it('renders error text from overlay events', async () => {
    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          text: 'Recognition failed',
          phase: 'error',
          revision: 2,
        },
      });
    });

    screen.getByText('Recognition failed');
  });
  it('renders polishing phase with custom styling and text', async () => {
    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'voice-typing-1',
          text: 'Polishing text',
          phase: 'polishing',
          revision: 3,
        },
      });
    });
    screen.getByText('AI polishing...');
    expect(screen.getByTestId('voice-typing-bubble').style.border).toContain('168, 85, 247');
  });

  it('renders situational context badge when contextMode is active', async () => {
    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'voice-typing-dev',
          text: 'let x = 1;',
          phase: 'segment',
          revision: 2,
          contextMode: 'developer',
        },
      });
    });

    const badge = screen.getByTestId('voice-typing-context-badge');
    expect(badge).toBeTruthy();
    expect(badge.textContent).toContain('Code');
  });

  it('renders selection rewrite badge when hasSelection is true', async () => {
    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'voice-typing-sel',
          text: 'Rewrite this section',
          phase: 'segment',
          revision: 2,
          hasSelection: true,
        },
      });
    });

    const badge = screen.getByTestId('voice-typing-selection-badge');
    expect(badge).toBeTruthy();
    expect(badge.textContent).toContain('Selection Rewrite');
  });

  it('uses the shared snapshot as the initial source of truth and ignores older revisions', async () => {
    mocks.invoke.mockImplementation(async (command: string): Promise<any> => {
      if (command === 'get_aux_window_state') {
        return {
          sessionId: 'voice-typing-9',
          text: 'Full sentence in snapshot',
          phase: 'segment',
          segmentId: 'seg-9',
          isFinal: false,
          revision: 4,
        };
      }
      return undefined;
    });

    render(<VoiceTypingOverlay />);

    expect(await screen.findByText('Full sentence in snapshot')).toBeTruthy();

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'voice-typing-9',
          text: '',
          phase: 'listening',
          revision: 3,
        },
      });
    });

    screen.getByText('Full sentence in snapshot');
  });

  it('falls back to polling the shared snapshot when no event arrives', async () => {
    vi.useFakeTimers();
    let snapshotCallCount = 0;
    mocks.invoke.mockImplementation(async (command: string): Promise<any> => {
      if (command === 'get_aux_window_state') {
        snapshotCallCount += 1;
        if (snapshotCallCount >= 2) {
          return {
            sessionId: 'voice-typing-10',
            text: 'Candidate from polling',
            phase: 'segment',
            segmentId: 'seg-10',
            isFinal: false,
            revision: 6,
          };
        }
        return null;
      }
      return undefined;
    });

    render(<VoiceTypingOverlay />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(140);
    });

    screen.getByText('Candidate from polling');
  });

  it('keeps snapshot polling active even if event listener registration fails', async () => {
    vi.useFakeTimers();
    let snapshotCallCount = 0;
    mocks.listen.mockRejectedValueOnce(new Error('listen failed'));
    mocks.currentWindowListen.mockRejectedValueOnce(new Error('window listen failed'));
    mocks.invoke.mockImplementation(async (command: string): Promise<any> => {
      if (command === 'get_aux_window_state') {
        snapshotCallCount += 1;
        if (snapshotCallCount >= 2) {
          return {
            sessionId: 'voice-typing-11',
            text: 'Visible after listen error',
            phase: 'segment',
            segmentId: 'seg-11',
            isFinal: false,
            revision: 7,
          };
        }
        return null;
      }
      return undefined;
    });

    render(<VoiceTypingOverlay />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(140);
    });

    screen.getByText('Visible after listen error');
    expect(mocks.loggerWarn).toHaveBeenCalledWith(
      '[useAuxWindowState] Failed to register app-level listener',
      expect.objectContaining({
        eventName: 'voice-typing:text',
        label: 'voice-typing',
      })
    );
    expect(mocks.loggerWarn).toHaveBeenCalledWith(
      '[useAuxWindowState] Failed to register current-window listener',
      expect.objectContaining({
        eventName: 'voice-typing:text',
        label: 'voice-typing',
      })
    );
  });

  it('resizes the auxiliary window to match measured content height', async () => {
    render(<VoiceTypingOverlay />);

    const root = screen.getByTestId('voice-typing-overlay-root');
    Object.defineProperty(root, 'getBoundingClientRect', {
      value: vi.fn(() => ({
        width: 240,
        height: 68,
        top: 0,
        left: 0,
        right: 240,
        bottom: 68,
        x: 0,
        y: 0,
        toJSON: () => undefined,
      })),
    });

    await act(async () => {
      resizeObserverCallback?.([], {} as ResizeObserver);
    });

    await waitFor(() => {
      expect(mocks.currentWindowSetSize).toHaveBeenCalledWith(
        expect.objectContaining({
          width: 400,
          height: 68,
        })
      );
    });
  });

  it('clamps and shifts window upwards if resizing would exceed bottom boundary of screen', async () => {
    mocks.currentWindowInnerPosition.mockResolvedValue({ x: 760, y: 900 });
    mocks.currentMonitor.mockResolvedValue({
      scaleFactor: 1,
      position: { x: 0, y: 0 },
      size: { width: 1920, height: 1080 },
      workArea: {
        position: { x: 0, y: 0 },
        size: { width: 1920, height: 1040 },
      },
    });

    render(<VoiceTypingOverlay />);

    const root = screen.getByTestId('voice-typing-overlay-root');
    Object.defineProperty(root, 'getBoundingClientRect', {
      value: vi.fn(() => ({
        width: 380,
        height: 320,
        top: 0,
        left: 0,
        right: 380,
        bottom: 320,
        x: 0,
        y: 0,
        toJSON: () => undefined,
      })),
    });

    await act(async () => {
      resizeObserverCallback?.([], {} as ResizeObserver);
    });

    await waitFor(() => {
      expect(mocks.currentWindowSetSize).toHaveBeenCalledWith(
        expect.objectContaining({
          width: 400,
          height: 320,
        })
      );
      expect(mocks.currentWindowSetPosition).toHaveBeenCalledWith(
        expect.objectContaining({
          x: 760,
          y: 704,
        })
      );
    });
  });

  it('renders the 5-bar audio waveform visualizer and cancel button', () => {
    render(<VoiceTypingOverlay />);
    const waveform = screen.getByTestId('voice-typing-waveform');
    expect(waveform).toBeDefined();
    expect(waveform.children.length).toBe(5);

    const cancelBtn = screen.getByTestId('voice-typing-cancel-btn');
    expect(cancelBtn).toBeDefined();
  });

  it('emits voice-typing:cancel event when cancel button is clicked', async () => {
    render(<VoiceTypingOverlay />);

    const cancelBtn = screen.getByTestId('voice-typing-cancel-btn');
    cancelBtn.click();

    expect(mocks.emit).toHaveBeenCalledWith('voice-typing:cancel');
  });

  it('emits voice-typing:cancel event when Escape key is pressed', async () => {
    render(<VoiceTypingOverlay />);

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));

    expect(mocks.emit).toHaveBeenCalledWith('voice-typing:cancel');
  });
  it('renders quick recall drawer in recall phase and handles reinject on keypress', async () => {
    useVoiceTypingHistoryStore.getState().clearHistory();
    useVoiceTypingHistoryStore.getState().addItem({
      rawText: 'First history entry',
      injectedText: 'First history entry',
      mode: 'raw',
    });

    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'recall-1',
          text: '',
          phase: 'recall',
          revision: 5,
        },
      });
    });

    expect(screen.getByTestId('voice-typing-recall-drawer')).toBeTruthy();
    screen.getByText('First history entry');

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: '1' }));
    });

    expect(mocks.emit).toHaveBeenCalledWith('voice-typing:reinject', {
      text: 'First history entry',
    });
  });

  it('renders quick recall drawer using history passed directly in overlay payload', async () => {
    // Store is empty in this window context
    useVoiceTypingHistoryStore.getState().clearHistory();
    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(0);

    render(<VoiceTypingOverlay />);

    await act(async () => {
      mocks.listenCallbacks['voice-typing:text']?.({
        payload: {
          sessionId: 'recall-2',
          text: '',
          phase: 'recall',
          revision: 6,
          history: [
            {
              id: 'payload-item-1',
              timestamp: Date.now(),
              rawText: 'History from main window payload',
              injectedText: 'History from main window payload',
              mode: 'raw',
            },
          ],
        },
      });
    });

    expect(screen.getByTestId('voice-typing-recall-drawer')).toBeTruthy();
    screen.getByText('History from main window payload');

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: '1' }));
    });

    expect(mocks.emit).toHaveBeenCalledWith('voice-typing:reinject', {
      text: 'History from main window payload',
    });
  });
});
