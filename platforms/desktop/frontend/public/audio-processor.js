class AudioProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    // 4096 samples matches the previous ScriptProcessorNode buffer size
    this.bufferSize = 4096;
    this.buffer = new Int16Array(this.bufferSize);
    this.bufferIndex = 0;
  }

  process(inputs, outputs, parameters) {
    const input = inputs[0];
    // input is array of channels (Float32Array)
    if (input && input.length > 0) {
      const channelData = input[0];

      for (let i = 0; i < channelData.length; i++) {
        // Convert Float32 (-1.0 to 1.0) to Int16
        const s = Math.max(-1, Math.min(1, channelData[i]));
        // Convert to PCM Int16
        this.buffer[this.bufferIndex++] = s < 0 ? s * 0x8000 : s * 0x7FFF;

        // When buffer is full, send it to the main thread
        if (this.bufferIndex >= this.bufferSize) {
          // Create a copy to send, so we can reuse our internal buffer
          const chunk = new Int16Array(this.buffer);

          // Post message with transfer to avoid copying the buffer data again during IPC
          this.port.postMessage(chunk, [chunk.buffer]);

          this.bufferIndex = 0;
        }
      }
    }

    // Return true to keep the processor alive
    return true;
  }
}

registerProcessor('audio-processor', AudioProcessor);
