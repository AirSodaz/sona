import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { LiveRecord } from '../LiveRecord';

const { mockSetLanguage, mockPrepare } = vi.hoisted(() => {
    return {
        mockSetLanguage: vi.fn(),
        mockPrepare: vi.fn().mockResolvedValue(undefined),
    };
});

// Mock transcription service
vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        start: vi.fn().mockResolvedValue(undefined),
        stop: vi.fn().mockResolvedValue(undefined),
        softStop: vi.fn().mockResolvedValue(undefined),
        sendAudioInt16: vi.fn(),
        setModelPath: vi.fn(),
        setLanguage: mockSetLanguage,
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),
        setPunctuationModelPath: vi.fn(),
        setCtcModelPath: vi.fn(),
        setVadModelPath: vi.fn(),
        setVadBufferSize: vi.fn(),
        prepare: mockPrepare,
        terminate: vi.fn().mockResolvedValue(undefined),
    }
}));

// Mock model service
vi.mock('../../services/modelService', () => ({
    modelService: {
        isITNModelInstalled: vi.fn().mockResolvedValue(false),
        getITNModelPath: vi.fn().mockResolvedValue('/path/to/itn'),
        getEnabledITNModelPaths: vi.fn().mockResolvedValue(['/path/to/itn']),
    }
}));

// Mock translation
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

// Mock icons
vi.mock('lucide-react', () => ({
    Pause: () => 'Pause',
    Play: () => 'Play',
    Square: () => 'Square',
    Mic: () => 'Mic',
    Monitor: () => 'Monitor',
    FileAudio: () => 'FileAudio',
}));

describe('LiveRecord Config Changes', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('should update transcription service language and trigger prepare when language config changes', async () => {
        const { useTranscriptStore } = await import('../../stores/transcriptStore');

        // Initial setup
        act(() => {
            useTranscriptStore.setState({
                config: {
                    ...useTranscriptStore.getState().config,
                    offlineModelPath: '/path/to/model',
                    language: 'en'
                }
            });
        });

        render(<LiveRecord />);

        // Wait for initial useEffect
        await act(async () => {
            await new Promise(resolve => setTimeout(resolve, 0));
        });

        // Initial prepare should have been called
        // We verify this to ensure the component is mounting and reacting to initial config correctly
        expect(mockPrepare).toHaveBeenCalled();

        mockSetLanguage.mockClear();
        mockPrepare.mockClear();

        // Change language
        await act(async () => {
            useTranscriptStore.setState({
                config: {
                    ...useTranscriptStore.getState().config,
                    language: 'zh'
                }
            });
            // Wait for useEffect to trigger
            await new Promise(resolve => setTimeout(resolve, 0));
        });

        // Expect setLanguage to be called with 'zh'
        expect(mockSetLanguage).toHaveBeenCalledWith('zh');
        // Expect prepare to be called
        expect(mockPrepare).toHaveBeenCalled();
    });
});
