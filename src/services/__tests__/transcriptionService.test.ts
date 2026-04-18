import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { transcriptionService } from '../transcriptionService';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn()
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(vi.fn()) // returns unlisten function
}));

describe('TranscriptionService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        // Reset internal states using public methods before each test
        transcriptionService.setModelPath('/mock/model/path');
        transcriptionService.setEnableITN(true);
        
        // Reset static global listeners to ensure listen is called in each test
        (transcriptionService.constructor as any).globalListeners.clear();
        (transcriptionService.constructor as any).instanceCallbacks.clear();
    });

    afterEach(async () => {
        await transcriptionService.stop();
    });

    it('starts the backend recognizer correctly', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();

        await transcriptionService.start(onSegment, onError);

        expect(listen).toHaveBeenCalledWith('recognizer-output-record', expect.any(Function));
        expect(invoke).toHaveBeenCalledWith('init_recognizer', {
            instanceId: 'record',
            modelPath: '/mock/model/path',
            numThreads: 4,
            enableItn: true,
            language: 'auto',
            punctuationModel: null,
            vadModel: null,
            vadBuffer: 5,
            modelType: 'sensevoice',
            fileConfig: undefined
        });
        expect(invoke).toHaveBeenCalledWith('start_recognizer', { instanceId: 'record' });
    });

    it('does not start if model path is missing', async () => {
        transcriptionService.setModelPath('');
        const onSegment = vi.fn();
        const onError = vi.fn();

        await transcriptionService.start(onSegment, onError);

        expect(onError).toHaveBeenCalledWith('Model path not configured');
        expect(invoke).not.toHaveBeenCalledWith('start_recognizer', expect.anything());
    });

    it('sends audio data to the backend', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();
        await transcriptionService.start(onSegment, onError);
        vi.mocked(invoke).mockClear();

        const audioData = new Int16Array([32767, 0, -32768]);
        await transcriptionService.sendAudioInt16(audioData);

        // Int16Array -> Uint8Array sends raw bytes
        const bytes = new Uint8Array(audioData.buffer, audioData.byteOffset, audioData.byteLength);

        expect(invoke).toHaveBeenCalledWith('feed_audio_chunk', {
            instanceId: 'record',
            samples: bytes
        });
    });

    it('handles segment data via listen event', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();
        await transcriptionService.start(onSegment, onError);

        // TranscriptionService ensures global bus, so listen is called
        const listenCalls = vi.mocked(listen).mock.calls;
        const listenCall = listenCalls.find(call => call[0] === 'recognizer-output-record');
        
        if (!listenCall) {
            throw new Error('listen was not called with recognizer-output-record');
        }
        
        const listenCallback = listenCall[1] as Function;

        const segmentData = {
            id: 'seg-1',
            text: 'Hello world',
            start: 0,
            end: 1.5,
            isFinal: true
        };

        // Simulate event payload
        listenCallback({ payload: segmentData });

        expect(onSegment).toHaveBeenCalledWith(expect.objectContaining({
            text: 'Hello world',
            start: 0,
            end: 1.5
        }));
    });

    it('stops the backend recognizer', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();
        await transcriptionService.start(onSegment, onError);
        vi.mocked(invoke).mockClear();

        await transcriptionService.stop();

        expect(invoke).toHaveBeenCalledWith('stop_recognizer', { instanceId: 'record' });
    });

    describe('Batch Transcription', () => {
        it('executes batch transcription with correct args', async () => {
            vi.mocked(invoke).mockImplementation((cmd) => {
                if (cmd === 'process_batch_file') return Promise.resolve([]);
                return Promise.resolve();
            });

            await transcriptionService.transcribeFile('/path/to/audio.wav');

            expect(invoke).not.toHaveBeenCalledWith('start_recognizer', expect.anything());

            expect(invoke).toHaveBeenCalledWith('process_batch_file', expect.objectContaining({
                filePath: '/path/to/audio.wav',
                saveToPath: null,
                modelPath: '/mock/model/path'
            }));
        });

        it('passes qwen3-asr model metadata for offline transcription', async () => {
            transcriptionService.setModelPath('/models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25');
            vi.mocked(invoke).mockImplementation((cmd) => {
                if (cmd === 'process_batch_file') return Promise.resolve([]);
                return Promise.resolve();
            });

            await transcriptionService.transcribeFile('/path/to/audio.wav');

            expect(invoke).toHaveBeenCalledWith('process_batch_file', expect.objectContaining({
                modelPath: '/models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25',
                modelType: 'qwen3-asr',
                fileConfig: {
                    convFrontend: 'conv_frontend.onnx',
                    encoder: 'encoder.int8.onnx',
                    decoder: 'decoder.int8.onnx',
                    tokenizer: 'tokenizer',
                },
            }));
            const batchCall = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === 'process_batch_file');
            expect((batchCall?.[1] as any)?.fileConfig).not.toHaveProperty('tokens');
        });

        it('parses batch results correctly', async () => {
            const mockSegments = [
                { text: 'Hello', start: 0, end: 1, isFinal: true },
                { text: 'World', start: 1, end: 2, isFinal: true }
            ];

            vi.mocked(invoke).mockImplementation((cmd) => {
                if (cmd === 'process_batch_file') return Promise.resolve(mockSegments);
                return Promise.resolve();
            });

            const results = await transcriptionService.transcribeFile('/path/to/audio.wav');

            expect(results).toHaveLength(2);
            expect(results[0].text).toBe('Hello');
            expect(results[1].text).toBe('World');
        });

        it('handles fallback to CPU on COREML_FAILURE', async () => {
            let attempt = 0;
            vi.mocked(invoke).mockImplementation((cmd) => {
                if (cmd === 'start_recognizer') return Promise.resolve();
                if (cmd === 'process_batch_file') {
                    attempt++;
                    if (attempt === 1) {
                        return Promise.reject(new Error('COREML_FAILURE'));
                    }
                    return Promise.resolve([]);
                }
                return Promise.resolve();
            });

            await transcriptionService.transcribeFile('/path/to/audio.wav');

            // The code currently retries and passes 'cpu' mapping which does not immediately affect the rust backend yet in the codebase but we test the retry mechanism itself.
            expect(attempt).toBe(2);
        });
    });

    describe('Filtering', () => {
        it('filters out segments with only a single period "." and isFinal: true in batch', async () => {
            const mockSegments = [
                { id: '1', text: '.', start: 0, end: 1, isFinal: true },
                { id: '2', text: 'Valid', start: 1, end: 2, isFinal: true }
            ];

            vi.mocked(invoke).mockImplementation((cmd) => {
                if (cmd === 'process_batch_file') return Promise.resolve(mockSegments);
                return Promise.resolve();
            });

            const onSegment = vi.fn();
            const results = await transcriptionService.transcribeFile('file', undefined, onSegment);

            // Should filter out the first segment
            expect(results).toHaveLength(1);
            expect(results[0].text).toBe('Valid');
            expect(onSegment).toHaveBeenCalledTimes(1);
            expect(onSegment).toHaveBeenCalledWith(expect.objectContaining({ text: 'Valid' }));
        });
    });
});
