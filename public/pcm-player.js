class PcmPlayer extends AudioWorkletProcessor {
  constructor() {
    super();
    // 16000 * 5 = 5 seconds buffer
    this.bufferSize = 16000 * 5;
    this.buffer = new Float32Array(this.bufferSize);
    this.readIndex = 0;
    this.writeIndex = 0;

    this.port.onmessage = (e) => {
      const int16 = e.data;
      if (!int16) return;

      for (let i = 0; i < int16.length; i++) {
        // Convert Int16 to Float32 (-1.0 to 1.0)
        const sample = int16[i] / 32768.0;
        this.buffer[this.writeIndex] = sample;
        this.writeIndex = (this.writeIndex + 1) % this.bufferSize;

        // Overflow check (if write meets read)
        if (this.writeIndex === this.readIndex) {
          // Drop oldest sample (advance read)
          this.readIndex = (this.readIndex + 1) % this.bufferSize;
        }
      }
    };
  }

  process(inputs, outputs, parameters) {
    const output = outputs[0];
    const channel = output[0];

    for (let i = 0; i < channel.length; i++) {
      if (this.readIndex !== this.writeIndex) {
        channel[i] = this.buffer[this.readIndex];
        this.readIndex = (this.readIndex + 1) % this.bufferSize;
      } else {
        channel[i] = 0; // Silence if underrun
      }
    }
    return true;
  }
}

registerProcessor('pcm-player', PcmPlayer);
