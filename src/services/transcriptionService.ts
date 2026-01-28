
import { Command, Child } from '@tauri-apps/plugin-shell';
import { resolveResource } from '@tauri-apps/api/path';
import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';
import { StreamLineBuffer } from '../utils/streamBuffer';

export type TranscriptionCallback = (segment: TranscriptSegment) => void;
export type ErrorCallback = (error: string) => void;

class TranscriptionService {
    private child: Child | null = null;
    private isRunning: boolean = false;
    private modelPath: string = '';

    private punctuationModelPath: string = '';
    private enableITN: boolean = true;
    private onSegment: TranscriptionCallback | null = null;
    private onReady: (() => void) | null = null;

    private onError: ErrorCallback | null = null;
    private vadModelPath: string = '';

    // Session Management for Isolation
    private totalSamplesSent: number = 0;
    private sessionStartTime: number = 0;
    private readonly SAMPLE_RATE = 16000;

    constructor() { }

    /**
     * Set the model path for sherpa-onnx
     */
    setModelPath(path: string) {
        console.log('[TranscriptionService] Setting model path:', path);
        this.modelPath = path;
    }



    /**
     * Set the punctuation model path
     */
    setPunctuationModelPath(path: string) {
        this.punctuationModelPath = path;
    }

    /**
     * Set the VAD model path
     */
    setVadModelPath(path: string) {
        this.vadModelPath = path;
    }

    /**
     * Set ITN enabled state
     */
    setEnableITN(enabled: boolean) {
        this.enableITN = enabled;
    }

    /**
     * Start a new transcription session
     * Sets the time barrier for valid segments
     */
    startSession() {
        // Calculate current "time" based on samples sent
        // Any segment ending before this time belongs to a previous session
        this.sessionStartTime = this.totalSamplesSent / this.SAMPLE_RATE;
        console.log('[TranscriptionService] Starting new session at time:', this.sessionStartTime);
    }

    /**
     * Start the transcription sidecar
     */
    async start(onSegment: TranscriptionCallback, onError: ErrorCallback, onReady?: () => void) {
        if (this.isRunning) {
            // Update callbacks
            this.onSegment = onSegment;
            this.onError = onError;
            if (onReady) this.onReady = onReady;
            return;
        }
        if (!this.modelPath) {
            onError('Model path not configured');
            return;
        }

        this.onSegment = onSegment;
        this.onError = onError;
        this.onReady = onReady || null;
        this.totalSamplesSent = 0;
        this.sessionStartTime = 0;

        console.log('Starting sidecar with model:', this.modelPath);

        try {
            const scriptPath = await resolveResource('sidecar/dist/index.mjs');

            const args = [
                scriptPath,
                '--mode', 'stream',
                '--model-path', this.modelPath,
                '--enable-itn', this.enableITN.toString()
            ];



            if (this.punctuationModelPath) {
                args.push('--punctuation-model', this.punctuationModelPath);
            }

            if (this.vadModelPath) {
                args.push('--vad-model', this.vadModelPath);
            }

            if (import.meta.env.DEV) {
                args.push('--allow-mock', 'true');
            }

            const command = Command.sidecar('binaries/node', args);

            // Buffer for stdout stream
            const stdoutBuffer = new StreamLineBuffer();

            command.on('close', (data) => {
                console.log(`Sidecar finished with code ${data.code} and signal ${data.signal}`);

                // Process any remaining data
                const remaining = stdoutBuffer.flush();
                remaining.forEach(line => this.handleOutput(line));

                this.isRunning = false;
                this.child = null;
            });

            command.on('error', (error) => {
                console.error(`Sidecar error: "${error}"`);
                if (this.onError) this.onError(`Process error: ${error}`);
                this.isRunning = false;
            });

            command.stdout.on('data', (chunk) => {
                if (typeof chunk === 'string') {
                    const lines = stdoutBuffer.process(chunk);
                    lines.forEach(line => this.handleOutput(line));
                }
            });

            command.stderr.on('data', (chunk) => {
                console.log(`[TranscriptionService] stderr: ${chunk}`);
            });

            this.child = await command.spawn();
            this.isRunning = true;
            console.log('[TranscriptionService] Sidecar started, PID:', this.child.pid);

        } catch (error) {
            console.error('Failed to spawn sidecar:', error);
            if (this.onError) this.onError(`Failed to start: ${error}`);
            this.isRunning = false;
        }
    }

    /**
     * Stop the transcription sidecar
     */
    async stop() {
        if (!this.child || !this.isRunning) return;

        try {
            await this.child.kill();
        } catch (error) {
            console.error('Failed to kill sidecar:', error);
        } finally {
            console.log('[TranscriptionService] Sidecar stopped');
            this.child = null;
            this.isRunning = false;
        }
    }

    /**
     * Send audio data to the sidecar
     * Expects 16kHz mono Float32 samples
     */
    async sendAudio(samples: Float32Array) {
        if (!this.child || !this.isRunning) return;

        try {
            // Convert Float32 to Int16 for the sidecar
            // Optimization: Manual clamping to avoid function call overhead in tight loop
            const len = samples.length;
            const buffer = new Int16Array(len);
            for (let i = 0; i < len; i++) {
                let s = samples[i];
                if (s > 1) s = 1;
                else if (s < -1) s = -1;
                buffer[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
            }

            // Command.write accepts string or Uint8Array
            // We need to send bytes. Uint8Array view of Int16Array
            const bytes = new Uint8Array(buffer.buffer);
            await this.child.write(bytes);
            this.totalSamplesSent += samples.length;
        } catch (error) {
            console.error('Failed to write audio to sidecar:', error);
        }
    }

    /**
     * Send pre-converted Int16 audio data to the sidecar
     */
    async sendAudioInt16(samples: Int16Array) {
        if (!this.child || !this.isRunning) return;

        try {
            // Command.write accepts string or Uint8Array
            // We need to send bytes. Uint8Array view of Int16Array
            // Use byteOffset and byteLength to handle cases where Int16Array is a view
            const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
            await this.child.write(bytes);
            this.totalSamplesSent += samples.length;
        } catch (error) {
            console.error('Failed to write audio to sidecar:', error);
        }
    }

    /**
     * Force the end of a segment by sending silence
     * This helps the VAD detect silence and finalize the current segment
     */
    async forceEndSegment() {
        if (!this.child || !this.isRunning) return;

        console.log('[TranscriptionService] Forcing end of segment with silence');
        // Send 1 second of silence
        const silence = new Float32Array(16000).fill(0);
        await this.sendAudio(silence);
    }

    /**
     * Transcribe an audio file in batch mode
     */
    async transcribeFile(filePath: string, onProgress?: (progress: number) => void): Promise<TranscriptSegment[]> {
        try {
            return await this._transcribeFileInternal(filePath, undefined, onProgress);
        } catch (error: any) {
            if (error.message === 'COREML_FAILURE') {
                console.warn('[TranscriptionService] CoreML failure detected. Retrying with CPU...');
                return await this._transcribeFileInternal(filePath, 'cpu', onProgress);
            }
            throw error;
        }
    }

    private async _transcribeFileInternal(filePath: string, provider?: string, onProgress?: (progress: number) => void): Promise<TranscriptSegment[]> {
        if (!this.modelPath) {
            throw new Error('Model path not configured');
        }

        console.log(`[TranscriptionService] Starting batch transcription for: ${filePath} (Provider: ${provider || 'auto'})`);

        return new Promise(async (resolve, reject) => {
            try {
                // Spawn sidecar in batch mode
                const scriptPath = await resolveResource('sidecar/dist/index.mjs');
                const args = [
                    scriptPath,
                    '--mode', 'batch',
                    '--file', filePath,
                    '--model-path', this.modelPath,
                    '--enable-itn', this.enableITN.toString()
                ];



                if (this.punctuationModelPath) {
                    args.push('--punctuation-model', this.punctuationModelPath);
                }

                if (provider) {
                    args.push('--provider', provider);
                }

                if (import.meta.env.DEV) {
                    args.push('--allow-mock', 'true');
                }

                const command = Command.sidecar('binaries/node', args);

                // Optimization: Use array of chunks instead of string concatenation
                // This prevents O(N^2) copying behavior for large outputs
                const stdoutChunks: string[] = [];
                const stderrChunks: string[] = [];
                const stderrStreamBuffer = new StreamLineBuffer();

                command.on('close', (data) => {
                    console.log(`[Batch] Sidecar finished with code ${data.code}`);

                    const stdoutBuffer = stdoutChunks.join('');
                    const stderrBuffer = stderrChunks.join('');

                    if (data.code === 0) {
                        // Check for silent CoreML failure
                        // CoreML errors are printed to stderr but sometimes the process exits with 0
                        if (!provider &&
                            stderrBuffer.includes('Error executing model') &&
                            stderrBuffer.includes('CoreMLExecutionProvider')) {

                            reject(new Error('COREML_FAILURE'));
                            return;
                        }

                        try {
                            // Find the JSON array in the output
                            // The script might output logs before the JSON

                            let segments: TranscriptSegment[] = [];

                            // Look for the line that looks like the start of our JSON array
                            // Or accumulate all JSON-like output (though the script outputs one big JSON array at the end)
                            // The script `console.log(JSON.stringify(segments, null, 2))` at the end.

                            // Let's try to parse the whole buffer first, but it might contain logs if not careful
                            // The script uses console.log for the final JSON and console.error for logs/progress
                            // But in case some logs slipped into stdout

                            // Simple heuristic: find the first '[' and last ']'
                            const start = stdoutBuffer.indexOf('[');
                            const end = stdoutBuffer.lastIndexOf(']');

                            if (start !== -1 && end !== -1) {
                                const jsonStr = stdoutBuffer.substring(start, end + 1);
                                segments = JSON.parse(jsonStr);
                                resolve(segments);
                            } else {
                                // Maybe it was empty?
                                resolve([]);
                            }
                        } catch (e) {
                            reject(new Error(`Failed to parse batch output: ${e}`));
                        }
                    } else {
                        reject(new Error(`Sidecar failed with code ${data.code}: ${stderrBuffer}`));
                    }
                });

                command.on('error', (error) => {
                    reject(new Error(`Process error: ${error}`));
                });

                command.stdout.on('data', (chunk) => {
                    if (typeof chunk === 'string') {
                        stdoutChunks.push(chunk);
                    }
                });

                command.stderr.on('data', (chunk) => {
                    if (typeof chunk === 'string') {
                        stderrChunks.push(chunk);

                        const lines = stderrStreamBuffer.process(chunk);
                        lines.forEach(line => {
                            // Parse progress from stderr lines
                            try {
                                const data = JSON.parse(line);
                                if (data.type === 'progress' && typeof data.percentage === 'number') {
                                    if (onProgress) onProgress(data.percentage);
                                }
                            } catch (e) {
                                // Not JSON or not progress
                            }
                            console.log(`[Batch] stderr: ${line}`);
                        });
                    }
                });

                await command.spawn();

            } catch (error) {
                reject(error);
            }
        });
    }

    private handleOutput(line: string) {
        try {
            const data = JSON.parse(line);

            // Check for segments
            if (data.type === 'ready') {
                console.log('[TranscriptionService] Sidecar ready');
                if (this.onReady) this.onReady();
            } else if (data.text && typeof data.start === 'number' && typeof data.end === 'number') {
                // Filter out stale segments from previous sessions
                // Use a small tolerance (0.1s) to allow for minor alignment differences
                if (this.sessionStartTime > 0 && data.end <= this.sessionStartTime + 0.1) {
                    console.warn(`[TranscriptionService] Dropping stale segment (End: ${data.end}, SessionStart: ${this.sessionStartTime}): ${data.text}`);
                    return;
                }

                const segment: TranscriptSegment = {
                    id: data.id || uuidv4(),
                    text: data.text,
                    start: data.start,
                    end: data.end,
                    isFinal: data.isFinal || false
                };

                if (this.onSegment) {
                    this.onSegment(segment);
                }
            } else if (data.error && this.onError) {
                this.onError(data.error);
            }
        } catch (e) {
            // Ignore non-JSON lines or partial chunks
            // console.debug('Failed to parse line:', line);
        }
    }
}

export const transcriptionService = new TranscriptionService();
