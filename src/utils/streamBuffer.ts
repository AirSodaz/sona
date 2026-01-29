/**
 * A buffer helper for processing stream chunks into lines.
 * Optimized to handle split chunks and multiple lines per chunk.
 */
export class StreamLineBuffer {
    private buffer: string = '';

    /**
     * Appends a chunk and returns complete lines.
     * The last incomplete line is kept in the buffer.
     *
     * @param chunk - The incoming string chunk.
     * @return Array of complete lines found in this chunk (plus accumulated buffer).
     */
    process(chunk: string): string[] {
        this.buffer += chunk;
        if (this.buffer.indexOf('\n') === -1) {
            return [];
        }

        const lines = this.buffer.split('\n');
        // The last element is the potentially incomplete line
        // (or empty string if the buffer ended with \n, which is what we want because split gives an empty string at the end)
        this.buffer = lines.pop() || '';

        return lines;
    }

    /**
     * Returns any remaining buffer content as the last line.
     * Useful when the stream ends.
     *
     * @return Array containing the remaining line if any, or empty array.
     */
    flush(): string[] {
        if (this.buffer.trim().length > 0) {
            const line = this.buffer;
            this.buffer = '';
            return [line];
        }
        return [];
    }
}
