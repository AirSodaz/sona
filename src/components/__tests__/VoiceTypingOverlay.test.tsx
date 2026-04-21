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

    return {
        listenCallbacks,
        unlisten: vi.fn(),
        listen: vi.fn((event: string, callback: (event: any) => void) => {
            listenCallbacks[event] = callback;
            return Promise.resolve(() => {
                delete listenCallbacks[event];
                mocks.unlisten();
            });
        }),
    };
});

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
        document.documentElement.style.background = '';
        document.body.style.background = '';
    });

    afterEach(() => {
        vi.clearAllMocks();
    });

    it('sets the document background to transparent for the overlay window', () => {
        render(<VoiceTypingOverlay />);

        expect(document.documentElement.style.background).toBe('transparent');
        expect(document.body.style.background).toBe('transparent');
    });

    it('renders incoming transcription text from Tauri events', async () => {
        render(<VoiceTypingOverlay />);

        expect(screen.getByText('正在聆听...')).toBeTruthy();

        await act(async () => {
            mocks.listenCallbacks['voice-typing:text']?.({
                payload: { sessionId: 'voice-typing-1', text: '测试转录结果', phase: 'segment', segmentId: 'seg-1', isFinal: false }
            });
        });

        expect(screen.getByText('测试转录结果')).toBeTruthy();
    });

    it('renders error text from Tauri events', async () => {
        render(<VoiceTypingOverlay />);

        await act(async () => {
            mocks.listenCallbacks['voice-typing:text']?.({
                payload: { sessionId: 'voice-typing-1', text: '识别失败', phase: 'error' }
            });
        });

        expect(screen.getByText('识别失败')).toBeTruthy();
    });
});
