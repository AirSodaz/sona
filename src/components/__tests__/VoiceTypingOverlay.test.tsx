import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VoiceTypingOverlay } from '../VoiceTypingOverlay';

vi.mock('react-i18next', async (importOriginal) => {
    const actual = await importOriginal<typeof import('react-i18next')>();
    return {
        ...actual,
        useTranslation: () => ({
            t: (key: string) => key === 'common.listening' ? '正在聆听...' : key,
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
            mocks.listenCallbacks['voice-typing:text']?.({ payload: { text: '测试转录结果' } });
        });

        expect(screen.getByText('测试转录结果')).toBeTruthy();
    });
});
