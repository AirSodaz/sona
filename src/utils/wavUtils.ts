
/**
 * Utility functions for working with WAV files.
 */

/**
 * Encodes raw PCM samples into a WAV file Blob.
 *
 * @param samples The raw PCM samples (Float32Array or Int16Array).
 * @param sampleRate The sample rate of the audio (e.g., 16000).
 * @param numChannels The number of audio channels (defaults to 1).
 * @param bitDepth The bit depth of the audio (16 or 32, defaults to 16).
 * @returns A Blob representing the WAV file.
 */
export function encodeWAV(
    samples: Float32Array | Int16Array,
    sampleRate: number,
    numChannels: number = 1,
    bitDepth: number = 16
): Blob {
    const bytesPerSample = bitDepth / 8;
    const blockAlign = numChannels * bytesPerSample;
    const byteRate = sampleRate * blockAlign;
    const dataSize = samples.length * bytesPerSample;
    const buffer = new ArrayBuffer(44 + dataSize);
    const view = new DataView(buffer);

    // RIFF identifier
    writeString(view, 0, 'RIFF');
    // file length
    view.setUint32(4, 36 + dataSize, true);
    // RIFF type
    writeString(view, 8, 'WAVE');
    // format chunk identifier
    writeString(view, 12, 'fmt ');
    // format chunk length
    view.setUint32(16, 16, true);
    // sample format (raw)
    view.setUint16(20, 1, true);
    // channel count
    view.setUint16(22, numChannels, true);
    // sample rate
    view.setUint32(24, sampleRate, true);
    // byte rate (sample rate * block align)
    view.setUint32(28, byteRate, true);
    // block align (channel count * bytes per sample)
    view.setUint16(32, blockAlign, true);
    // bits per sample
    view.setUint16(34, bitDepth, true);
    // data chunk identifier
    writeString(view, 36, 'data');
    // data chunk length
    view.setUint32(40, dataSize, true);

    // write the PCM samples
    if (samples instanceof Float32Array) {
        floatTo16BitPCM(view, 44, samples);
    } else {
        // Int16Array
        int16To16BitPCM(view, 44, samples);
    }

    return new Blob([view], { type: 'audio/wav' });
}

function writeString(view: DataView, offset: number, string: string) {
    for (let i = 0; i < string.length; i++) {
        view.setUint8(offset + i, string.charCodeAt(i));
    }
}

function floatTo16BitPCM(output: DataView, offset: number, input: Float32Array) {
    for (let i = 0; i < input.length; i++, offset += 2) {
        const s = Math.max(-1, Math.min(1, input[i]));
        output.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7FFF, true);
    }
}

function int16To16BitPCM(output: DataView, offset: number, input: Int16Array) {
    for (let i = 0; i < input.length; i++, offset += 2) {
        output.setInt16(offset, input[i], true);
    }
}
