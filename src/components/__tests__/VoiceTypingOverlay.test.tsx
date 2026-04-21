import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VoiceTypingOverlay } from '../VoiceTypingOverlay';

vi.mock('react-i18next', async (importOriginal) => {
    const actual = await importOriginal<typeof import('react-i18next')>();
    return {
        ...actual,
        useTranslation: () => ({
            t: (key: string) => {
                if (key === 'common.listening') return '正在聆听...';
                if (key === 'common.preparing') return '正在准备...';
                return key;
            },
        }),
    };
});

const mocks = vi.hoisted(() => {
    const listenCallbacks: Record<string, (event: any) => void> = {};
    let storeKeyChangeCallback: ((value: any) => void) | null = null;
    const unlisten = vi.fn();
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

    return {
        currentWindowInnerSize,
        currentWindowScaleFactor,
        currentWindowSetSize,
        invoke,
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
    }),
}));

vi.mock('@tauri-apps/api/dpi', () => ({
    PhysicalSize: class MockPhysicalSize {
        constructor(
            public width: number,
            public height: number
        ) { }
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

        global.ResizeObserver = class {
            constructor(callback: ResizeObserverCallback) {
                resizeObserverCallback = callback;
            }

            observe() { }

            disconnect() { }

            unobserve() { }
        } as typeof ResizeObserver;
    });

    afterEach(() => {
        vi.clearAllMocks();
    });

    it('shows the listening placeholder by default and sets a transparent background', () => {
        render(<VoiceTypingOverlay />);

        expect(screen.getByText('正在聆听...')).toBeTruthy();
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
                    text: '测试转录结果',
                    phase: 'segment',
                    segmentId: 'seg-1',
                    isFinal: false,
                    revision: 1,
                },
            });
        });

        expect(screen.getByText('测试转录结果')).toBeTruthy();
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

        expect(screen.getByText('。')).toBeTruthy();
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
                    sessionId: 'voice-typing-1',
                    text: '识别失败',
                    phase: 'error',
                    revision: 2,
                },
            });
        });

        expect(screen.getByText('识别失败')).toBeTruthy();
    });

    it('uses the shared snapshot as the initial source of truth and ignores older revisions', async () => {
        mocks.invoke.mockImplementation(async (command: string): Promise<any> => {
            if (command === 'get_aux_window_state') {
                return {
                    sessionId: 'voice-typing-9',
                    text: '快照里的整句',
                    phase: 'segment',
                    segmentId: 'seg-9',
                    isFinal: false,
                    revision: 4,
                };
            }
            return undefined;
        });

        render(<VoiceTypingOverlay />);

        expect(await screen.findByText('快照里的整句')).toBeTruthy();

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

        expect(screen.getByText('快照里的整句')).toBeTruthy();
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
                        text: '轮询拿到的候选条',
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

        expect(screen.getByText('轮询拿到的候选条')).toBeTruthy();
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
                        text: '监听失败后仍可见',
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

        expect(screen.getByText('监听失败后仍可见')).toBeTruthy();
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
});
