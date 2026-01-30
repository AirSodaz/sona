import { describe, it, expect } from 'vitest';
import { StreamLineBuffer } from './streamBuffer';

// Old logic for comparison
class OldStreamLineBuffer {
    private buffer: string = '';

    process(chunk: string): string[] {
        this.buffer += chunk;
        if (this.buffer.indexOf('\n') === -1) {
            return [];
        }

        const lines = this.buffer.split('\n');
        this.buffer = lines.pop() || '';

        return lines;
    }

    flush(): string[] {
        if (this.buffer.trim().length > 0) {
            const line = this.buffer;
            this.buffer = '';
            return [line];
        }
        return [];
    }
}

describe('StreamLineBuffer Performance', () => {
    // Generate a large line split into many small chunks
    const CHUNK_SIZE = 10;
    const NUM_CHUNKS = 5000; // Total 50k chars
    const chunks: string[] = [];
    const payload = 'x'.repeat(CHUNK_SIZE);

    for (let i = 0; i < NUM_CHUNKS; i++) {
        chunks.push(payload);
    }
    // Add a newline at the very end
    chunks.push('\n');

    it('benchmarks actual (optimized) vs old logic', () => {
        const optimized = new StreamLineBuffer(); // Imported from optimized file
        const old = new OldStreamLineBuffer();

        const startOld = performance.now();
        for (const chunk of chunks) {
            old.process(chunk);
        }
        old.flush();
        const endOld = performance.now();

        const startOptimized = performance.now();
        for (const chunk of chunks) {
            optimized.process(chunk);
        }
        optimized.flush();
        const endOptimized = performance.now();

        console.log(`Old Logic: ${(endOld - startOld).toFixed(3)}ms`);
        console.log(`New Logic (Actual): ${(endOptimized - startOptimized).toFixed(3)}ms`);

        // Correctness check
        const bResult = new OldStreamLineBuffer();
        const oResult = new StreamLineBuffer();

        let bLines: string[] = [];
        let oLines: string[] = [];

        for (const chunk of chunks) {
            bLines = bLines.concat(bResult.process(chunk));
            oLines = oLines.concat(oResult.process(chunk));
        }
        bLines = bLines.concat(bResult.flush());
        oLines = oLines.concat(oResult.flush());

        expect(oLines).toEqual(bLines);
        expect(endOptimized - startOptimized).toBeLessThan(endOld - startOld);
    });
});
