import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { transcriptionService } from '../transcriptionService';
import { Command } from '@tauri-apps/plugin-shell';
import { resolveResource } from '@tauri-apps/api/path';

// Mock dependencies
vi.mock('@tauri-apps/plugin-shell', () => {
    const EventEmitter = require('events');

    class MockChild {
        pid = 12345;
        kill = vi.fn().mockResolvedValue(undefined);
        write = vi.fn().mockResolvedValue(undefined);
    }

    class MockCommand extends EventEmitter {
        stdout = new EventEmitter();
        stderr = new EventEmitter();
        spawn = vi.fn().mockResolvedValue(new MockChild());

        static sidecar = vi.fn((_bin, _args) => {
            return new MockCommand();
        });
    }

    return {
        Command: MockCommand,
        Child: MockChild
    };
});

vi.mock('@tauri-apps/api/path', () => ({
    resolveResource: vi.fn().mockResolvedValue('/mock/resource/path/sidecar.mjs'),
}));

vi.mock('uuid', () => ({
    v4: () => 'mock-uuid'
}));

describe('TranscriptionService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        transcriptionService.stop();
        transcriptionService.setModelPath('/mock/model/path');
        transcriptionService.setITNModelPaths([]);
        transcriptionService.setPunctuationModelPath('');
        transcriptionService.setEnableITN(true);
    });

    afterEach(async () => {
        await transcriptionService.stop();
    });

    it('starts the sidecar process correctly', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();

        await transcriptionService.start(onSegment, onError);

        expect(resolveResource).toHaveBeenCalledWith('sidecar/dist/index.mjs');
        expect(Command.sidecar).toHaveBeenCalledWith('binaries/node', expect.arrayContaining([
            '/mock/resource/path/sidecar.mjs',
            '--mode', 'stream',
            '--model-path', '/mock/model/path',
            '--enable-itn', 'true'
        ]));
    });

    it('does not start if model path is missing', async () => {
        transcriptionService.setModelPath('');
        const onSegment = vi.fn();
        const onError = vi.fn();

        await transcriptionService.start(onSegment, onError);

        expect(onError).toHaveBeenCalledWith('Model path not configured');
        expect(Command.sidecar).not.toHaveBeenCalled();
    });

    it('sends audio data to the sidecar', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();
        await transcriptionService.start(onSegment, onError);

        const mockCommandInstance = vi.mocked(Command.sidecar).mock.results[0].value;
        const mockChild = await mockCommandInstance.spawn.mock.results[0].value;

        const audioData = new Int16Array([1, 2, 3, 4]);
        await transcriptionService.sendAudioInt16(audioData);

        expect(mockChild.write).toHaveBeenCalledWith(expect.any(Uint8Array));
        const writtenBytes = mockChild.write.mock.calls[0][0];
        expect(writtenBytes.length).toBe(8);
    });

    it('handles stdout segment data', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();
        await transcriptionService.start(onSegment, onError);

        const mockCommandInstance = vi.mocked(Command.sidecar).mock.results[0].value;

        const segmentData = {
            id: 'seg-1',
            text: 'Hello world',
            start: 0,
            end: 1.5,
            isFinal: true
        };

        mockCommandInstance.stdout.emit('data', JSON.stringify(segmentData) + '\n');

        expect(onSegment).toHaveBeenCalledWith(expect.objectContaining({
            text: 'Hello world',
            start: 0,
            end: 1.5
        }));
    });

    it('handles stdout error messages', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();
        await transcriptionService.start(onSegment, onError);

        const mockCommandInstance = vi.mocked(Command.sidecar).mock.results[0].value;

        mockCommandInstance.stdout.emit('data', JSON.stringify({ error: 'Something went wrong' }) + '\n');

        expect(onError).toHaveBeenCalledWith('Something went wrong');
    });

    it('stops the sidecar process', async () => {
        const onSegment = vi.fn();
        const onError = vi.fn();
        await transcriptionService.start(onSegment, onError);

        const mockCommandInstance = vi.mocked(Command.sidecar).mock.results[0].value;
        const mockChild = await mockCommandInstance.spawn.mock.results[0].value;

        await transcriptionService.stop();

        expect(mockChild.kill).toHaveBeenCalled();
    });

    describe('Batch Transcription', () => {
        it('executes batch transcription with correct args', async () => {
            const promise = transcriptionService.transcribeFile('/path/to/audio.wav');

            // Wait a tick to ensure synchronous part of transcribeFile has run
            await new Promise(resolve => setTimeout(resolve, 0));

            const mockCommandInstance = vi.mocked(Command.sidecar).mock.results[0].value;

            mockCommandInstance.emit('close', { code: 0, signal: null });

            await promise;

            expect(Command.sidecar).toHaveBeenCalledWith('binaries/node', expect.arrayContaining([
                expect.stringContaining('sidecar.mjs'),
                '--mode', 'batch',
                '--file', '/path/to/audio.wav',
                '--model-path', '/mock/model/path'
            ]));
        });

        it('parses batch results correctly', async () => {
            const resultPromise = transcriptionService.transcribeFile('/path/to/audio.wav');

            // Wait a tick
            await new Promise(resolve => setTimeout(resolve, 0));

            const mockCommandInstance = vi.mocked(Command.sidecar).mock.results[0].value;

            // Simulate output
            const segment1 = { text: 'Hello', start: 0, end: 1, isFinal: true };
            const segment2 = { text: 'World', start: 1, end: 2, isFinal: true };

            mockCommandInstance.stdout.emit('data', JSON.stringify(segment1) + '\n');
            mockCommandInstance.stdout.emit('data', JSON.stringify(segment2) + '\n');

            mockCommandInstance.emit('close', { code: 0 });

            const results = await resultPromise;
            expect(results).toHaveLength(2);
            expect(results[0].text).toBe('Hello');
            expect(results[1].text).toBe('World');
        });

        it('handles fallback to CPU on CoreML failure', async () => {
            const resultPromise = transcriptionService.transcribeFile('/path/to/audio.wav');

            // Wait a tick
            await new Promise(resolve => setTimeout(resolve, 0));

            // First attempt (failed)
            const mockCommandInstance1 = vi.mocked(Command.sidecar).mock.results[0].value;

            mockCommandInstance1.stderr.emit('data', 'Error executing model ... CoreMLExecutionProvider\n');
            mockCommandInstance1.emit('close', { code: 0 });

            // Wait for retry logic
            await new Promise(resolve => setTimeout(resolve, 10));

            // Second attempt (success)
            const mockCommandInstance2 = vi.mocked(Command.sidecar).mock.results[1].value;
            mockCommandInstance2.emit('close', { code: 0 });

            await resultPromise;

            expect(Command.sidecar).toHaveBeenCalledTimes(2);
            const secondCallArgs = vi.mocked(Command.sidecar).mock.calls[1][1];
            expect(secondCallArgs).toContain('--provider');
            expect(secondCallArgs).toContain('cpu');
        });
    });

    describe('Alignment', () => {
        it('executes alignment with correct args', async () => {
            transcriptionService.setCtcModelPath('/mock/ctc/model');

            const segment = {
                id: 'seg-1',
                text: 'Hello',
                start: 0,
                end: 1,
                isFinal: true,
                tokens: ['H', 'e', 'l', 'l', 'o'],
                timestamps: [0, 0.2, 0.4, 0.6, 0.8]
            };

            const promise = transcriptionService.alignSegment(segment, '/path/to/audio.wav');

            // Wait a tick to ensure synchronous part has run
            await new Promise(resolve => setTimeout(resolve, 0));

            const mockCommandInstance = vi.mocked(Command.sidecar).mock.results[0].value;

            // Simulate output
            const resultData = {
                tokens: ['H', 'e', 'l', 'l', 'o'],
                timestamps: [0.1, 0.3, 0.5, 0.7, 0.9],
                durations: [0.2, 0.2, 0.2, 0.2, 0.2],
                ctcText: 'Hello'
            };

            mockCommandInstance.stdout.emit('data', JSON.stringify(resultData) + '\n');
            mockCommandInstance.emit('close', { code: 0 });

            const result = await promise;

            expect(Command.sidecar).toHaveBeenCalledWith('binaries/node', expect.arrayContaining([
                expect.stringContaining('sidecar.mjs'),
                '--mode', 'align',
                '--file', '/path/to/audio.wav',
                '--ctc-model', '/mock/ctc/model',
                '--start-time', '0',
                '--end-time', '1'
            ]));

            expect(result).toEqual(resultData);
        });
    });
});
