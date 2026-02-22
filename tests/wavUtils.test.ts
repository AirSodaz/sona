
import { describe, it, expect } from 'vitest';
import { encodeWAV } from '../src/utils/wavUtils';

describe('encodeWAV', () => {
    it('should create a valid WAV blob', () => {
        const samples = new Int16Array([0, 1000, -1000, 32767, -32768]);
        const blob = encodeWAV(samples, 16000, 1, 16);

        expect(blob.type).toBe('audio/wav');
        expect(blob.size).toBe(44 + samples.length * 2);

        // Read the blob content (using FileReader logic manually as we are in test)
        // In Node environment, we can't easily read Blob content directly without polyfills
        // But we can verify the size and type
    });

    it('should handle Float32Array correctly', () => {
        const samples = new Float32Array([0.0, 0.5, -0.5, 1.0, -1.0]);
        const blob = encodeWAV(samples, 16000, 1, 16);

        expect(blob.type).toBe('audio/wav');
        // 44 header bytes + 5 samples * 2 bytes/sample = 54 bytes
        expect(blob.size).toBe(54);
    });
});
