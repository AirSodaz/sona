import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export interface AudioDevice {
    id: string;
    label: string;
}

type AudioCallback = (data: Int16Array) => void;

export class AudioCaptureService {
    private unlisten: UnlistenFn | null = null;
    private isCapturing = false;
    private callbacks: Set<AudioCallback> = new Set();

    private refCount = 0;
    private currentDeviceId: string | null = null;

    /**
     * Gets a list of available audio devices from FFmpeg.
     */
    async getDevices(): Promise<AudioDevice[]> {
        try {
            return await invoke<AudioDevice[]>('get_audio_devices');
        } catch (error) {
            console.error('Failed to list audio devices:', error);
            return [];
        }
    }

    /**
     * Starts capturing audio from the specified device using FFmpeg.
     * Audio data is automatically forwarded to subscribers.
     */
    async startCapture(deviceId: string): Promise<void> {
        if (this.isCapturing) {
            if (this.currentDeviceId === deviceId) {
                console.log('Already capturing this device, incrementing ref count.');
                this.refCount++;
                return;
            }
            console.warn('Capture running on different device, stopping previous session.');
            const preservedRefCount = this.refCount;
            await this.stopCapture(true);
            this.refCount = preservedRefCount;
        }

        console.log(`Starting FFmpeg capture on device: ${deviceId}`);
        this.currentDeviceId = deviceId;
        this.refCount++;

        // Listen for audio packets from Rust
        this.unlisten = await listen<any>('audio-packet', (event) => {
            // event.payload comes as number[] (serialized Vec<u8>)
            try {
                const data = event.payload;
                let uint8: Uint8Array;

                if (Array.isArray(data)) {
                     uint8 = new Uint8Array(data);
                } else {
                    console.warn('Unexpected payload format', data);
                    return;
                }

                // Convert to Int16Array (16-bit PCM)
                const int16 = new Int16Array(uint8.buffer, uint8.byteOffset, uint8.byteLength / 2);

                // Notify listeners
                this.callbacks.forEach(cb => cb(int16));

            } catch (e) {
                console.error('Error processing audio packet:', e);
            }
        });

        try {
            await invoke('start_audio_capture', { deviceId });
            this.isCapturing = true;
        } catch (error) {
            console.error('Failed to start audio capture:', error);
            if (this.unlisten) {
                this.unlisten();
                this.unlisten = null;
            }
            this.isCapturing = false;
            this.refCount = 0;
            this.currentDeviceId = null;
            throw error;
        }
    }

    /**
     * Stops the FFmpeg audio capture process.
     * @param force Force stop ignoring ref count (e.g. device switch)
     */
    async stopCapture(force = false): Promise<void> {
        if (!this.isCapturing) return;

        if (!force) {
            this.refCount--;
            if (this.refCount > 0) {
                console.log('Capture stopped (ref decrement), remaining:', this.refCount);
                return;
            }
        }

        // Always try to stop if we think we are capturing, or just to be safe
        try {
            await invoke('stop_audio_capture');
        } catch (error) {
            console.error('Failed to stop audio capture:', error);
        } finally {
            if (this.unlisten) {
                this.unlisten();
                this.unlisten = null;
            }
            this.isCapturing = false;
            this.refCount = 0;
            this.currentDeviceId = null;
        }
    }

    /**
     * Checks if capture is currently active.
     */
    isActive(): boolean {
        return this.isCapturing;
    }

    /**
     * Subscribe to audio data events.
     * @returns unsubscribe function
     */
    onAudio(callback: AudioCallback): () => void {
        this.callbacks.add(callback);
        return () => { this.callbacks.delete(callback); };
    }
}

export const audioCaptureService = new AudioCaptureService();
