
import { Command, Child } from '@tauri-apps/plugin-shell';
import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';

export type TranscriptionCallback = (segment: TranscriptSegment) => void;
export type ErrorCallback = (error: string) => void;

class TranscriptionService {
    private child: Child | null = null;
    private isRunning: boolean = false;
    private modelPath: string = '';
    private enableITN: boolean = true;
    private onSegment: TranscriptionCallback | null = null;
    private onError: ErrorCallback | null = null;

    constructor() { }

    /**
     * Set the model path for sherpa-onnx
     */
    setModelPath(path: string) {
        console.log('[TranscriptionService] Setting model path:', path);
        this.modelPath = path;
    }

    /**
     * Set ITN enabled state
     */
    setEnableITN(enabled: boolean) {
        this.enableITN = enabled;
    }

    /**
     * Start the transcription sidecar
     */
    async start(onSegment: TranscriptionCallback, onError: ErrorCallback) {
        if (this.isRunning) return;
        if (!this.modelPath) {
            onError('Model path not configured');
            return;
        }

        this.onSegment = onSegment;
        this.onError = onError;

        console.log('Starting sidecar with model:', this.modelPath);

        try {
            // Spawn the node process
            // Working directory in Tauri dev is src-tauri, so use relative path from there
            const scriptPath = 'sidecar/sherpa-recognizer.js';

            // NOTE: In production, this needs to be handled differently (e.g. sidecar binary or resource)
            // For this dev environment implementation, we use 'node' command
            const args = [
                scriptPath,
                '--mode', 'stream',
                '--model-path', this.modelPath,
                '--enable-itn', this.enableITN.toString()
            ];

            if (import.meta.env.DEV) {
                args.push('--allow-mock', 'true');
            }

            const command = Command.create('node', args);

            command.on('close', (data) => {
                console.log(`Sidecar finished with code ${data.code} and signal ${data.signal}`);
                this.isRunning = false;
                this.child = null;
            });

            command.on('error', (error) => {
                console.error(`Sidecar error: "${error}"`);
                if (this.onError) this.onError(`Process error: ${error}`);
                this.isRunning = false;
            });

            command.stdout.on('data', (line) => {
                this.handleOutput(line);
            });

            command.stderr.on('data', (line) => {
                console.log(`[TranscriptionService] stderr: ${line}`);
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
     * Expects 16kHz mono Float32 or Int16 samples
     */
    async sendAudio(samples: Float32Array) {
        if (!this.child || !this.isRunning) return;

        try {
            // Convert Float32 to Int16 for the sidecar
            const buffer = new Int16Array(samples.length);
            for (let i = 0; i < samples.length; i++) {
                const s = Math.max(-1, Math.min(1, samples[i]));
                buffer[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
            }

            // Command.write accepts string or Uint8Array
            // We need to send bytes. Uint8Array view of Int16Array
            const bytes = new Uint8Array(buffer.buffer);
            await this.child.write(bytes);
        } catch (error) {
            console.error('Failed to write audio to sidecar:', error);
        }
    }

    /**
     * Transcribe an audio file in batch mode
     */
    async transcribeFile(filePath: string): Promise<TranscriptSegment[]> {
        if (!this.modelPath) {
            throw new Error('Model path not configured');
        }

        console.log('[TranscriptionService] Starting batch transcription for:', filePath);

        return new Promise(async (resolve, reject) => {
            try {
                // Spawn sidecar in batch mode
                const scriptPath = 'sidecar/sherpa-recognizer.js';
                const args = [
                    scriptPath,
                    '--mode', 'batch',
                    '--file', filePath,
                    '--model-path', this.modelPath,
                    '--enable-itn', this.enableITN.toString()
                ];

                if (import.meta.env.DEV) {
                    args.push('--allow-mock', 'true');
                }

                const command = Command.create('node', args);

                let stdoutBuffer = '';
                let stderrBuffer = '';

                command.on('close', (data) => {
                    console.log(`[Batch] Sidecar finished with code ${data.code}`);
                    if (data.code === 0) {
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

                command.stdout.on('data', (line) => {
                    stdoutBuffer += line + '\n';
                });

                command.stderr.on('data', (line) => {
                    stderrBuffer += line + '\n';
                    // We could parse progress from stderr here if we wanted
                    console.log(`[Batch] stderr: ${line}`);
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
            if (data.text && typeof data.start === 'number' && typeof data.end === 'number') {
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
