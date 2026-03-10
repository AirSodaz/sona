import { vi, describe, it, expect, beforeEach } from 'vitest';
import { polishService } from '../polishService';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { TranscriptSegment } from '../../types/transcript';
import { invoke } from '@tauri-apps/api/core';

// Mock invoke
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

// Mock store
vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: {
        getState: vi.fn(),
        setState: vi.fn(),
        subscribe: vi.fn(),
    },
}));

describe('PolishService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        // Setup default store state
        (useTranscriptStore.getState as any).mockReturnValue({
            config: {
                aiApiKey: 'test-key',
                aiBaseUrl: 'test-url',
                aiModel: 'test-model',
                aiServiceType: 'openai',
            },
            segments: [],
            updateSegment: vi.fn(),
            updateAiState: vi.fn(),
            sourceHistoryId: null,
        });
    });

    it('polishSegments calls AI and parses response', async () => {
        const segments: TranscriptSegment[] = [
            { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
            { id: '2', start: 1, end: 2, text: 'world', isFinal: true },
        ];

        const mockResponse = JSON.stringify([
            { id: '1', text: 'Hello' },
            { id: '2', text: 'World' },
        ]);

        (invoke as any).mockResolvedValue(mockResponse);

        const onChunk = vi.fn();

        await polishService.polishSegments(segments, onChunk);

        expect(invoke).toHaveBeenCalledWith('call_ai_model', expect.objectContaining({
            apiKey: 'test-key',
            input: expect.stringContaining('hello'),
        }));

        expect(onChunk).toHaveBeenCalledWith([
            { id: '1', text: 'Hello' },
            { id: '2', text: 'World' },
        ]);
    });

    it('polishSegments handles AI error', async () => {
        const segments: TranscriptSegment[] = [
            { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
        ];

        (invoke as any).mockRejectedValue(new Error('AI Error'));

        await expect(polishService.polishSegments(segments)).rejects.toThrow('AI Error');
    });

    it('polishTranscript updates store', async () => {
        const segments: TranscriptSegment[] = [
            { id: '1', start: 0, end: 1, text: 'hello', isFinal: true },
        ];

        const mockStore = {
            config: {
                aiApiKey: 'test-key',
                aiBaseUrl: 'test-url',
                aiModel: 'test-model',
                aiServiceType: 'openai',
            },
            segments: segments,
            updateSegment: vi.fn(),
            updateAiState: vi.fn(),
            sourceHistoryId: null,
        };

        (useTranscriptStore.getState as any).mockReturnValue(mockStore);

        const mockResponse = JSON.stringify([
            { id: '1', text: 'Hello' },
        ]);
        (invoke as any).mockResolvedValue(mockResponse);

        await polishService.polishTranscript();

        expect(mockStore.updateAiState).toHaveBeenCalledWith({ isPolishing: true, polishProgress: 0 }, 'current');
        expect(mockStore.updateSegment).toHaveBeenCalledWith('1', { text: 'Hello' });
        expect(mockStore.updateAiState).toHaveBeenCalledWith({ isPolishing: false, polishProgress: 0 }, 'current');
    });
});
