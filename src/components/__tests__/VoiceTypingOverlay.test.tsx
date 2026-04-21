import { act, render, screen } from '@testing-library/react';
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

    return {
        invoke,
        listen,
        listenCallbacks,
        unlisten,
    };
});

vi.mock('@tauri-apps/api/core', () => ({
    invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.listen,
}));

vi.mock('../../utils/logger', () => ({
    logger: {
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
        debug: vi.fn(),
    },
}));

describe('VoiceTypingOverlay', () => {
    beforeEach(() => {
        for (const key of Object.keys(mocks.listenCallbacks)) {
            delete mocks.listenCallbacks[key];
        }
        mocks.invoke.mockImplementation(async (command: string): Promise<any> => {
            if (command === 'get_aux_window_state') {
                return null;
            }
            return undefined;
        });
        document.documentElement.style.background = '';
        document.body.style.background = '';
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
});
