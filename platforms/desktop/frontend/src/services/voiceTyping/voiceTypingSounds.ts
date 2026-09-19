/**
 * Voice Typing Earcons (Synthesized Sound Effects)
 *
 * Implements subtle, low-latency audio feedback (chimes) for voice typing lifecycle events:
 * - start: ascending chime indicating listening activated
 * - commit: light confirmation chime indicating text injection completed
 * - cancel: gentle descending tone indicating speech discarded
 * - error: subtle muted alert
 *
 * Built with the Web Audio API for zero disk I/O, zero dependencies, and instant playback.
 */

type EarconType = 'start' | 'commit' | 'cancel' | 'error';

class VoiceTypingSoundPlayer {
  private audioContext: AudioContext | null = null;
  private isMuted = false;

  public setMuted(muted: boolean): void {
    this.isMuted = muted;
  }

  public resetForTest(): void {
    this.audioContext = null;
    this.isMuted = false;
  }
  private getAudioContext(): AudioContext | null {
    const AudioCtx =
      (typeof window !== 'undefined' &&
        // biome-ignore lint/suspicious/noExplicitAny: webkitAudioContext fallback for older webviews
        (window.AudioContext || (window as any).webkitAudioContext)) ||
      (typeof AudioContext !== 'undefined' ? AudioContext : null);
    if (!AudioCtx) {
      return null;
    }

    try {
      if (!this.audioContext || this.audioContext.state === 'closed') {
        this.audioContext = new AudioCtx();
      }
      if (this.audioContext.state === 'suspended') {
        void this.audioContext.resume();
      }
      return this.audioContext;
    } catch {
      return null;
    }
  }

  public play(type: EarconType): void {
    if (this.isMuted) {
      return;
    }

    try {
      const ctx = this.getAudioContext();
      if (!ctx) {
        return;
      }

      const now = ctx.currentTime;

      switch (type) {
        case 'start': {
          // Ascending two-tone chime (F5 698Hz -> C6 1046Hz, ~90ms)
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();

          osc.type = 'sine';
          osc.frequency.setValueAtTime(698.46, now);
          osc.frequency.exponentialRampToValueAtTime(1046.5, now + 0.08);

          gain.gain.setValueAtTime(0.12, now);
          gain.gain.exponentialRampToValueAtTime(0.001, now + 0.09);

          osc.connect(gain);
          gain.connect(ctx.destination);

          osc.start(now);
          osc.stop(now + 0.09);
          break;
        }

        case 'commit': {
          // Confirmation chime (A5 880Hz -> D6 1175Hz, ~60ms)
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();

          osc.type = 'sine';
          osc.frequency.setValueAtTime(880, now);
          osc.frequency.exponentialRampToValueAtTime(1174.66, now + 0.06);

          gain.gain.setValueAtTime(0.1, now);
          gain.gain.exponentialRampToValueAtTime(0.001, now + 0.07);

          osc.connect(gain);
          gain.connect(ctx.destination);

          osc.start(now);
          osc.stop(now + 0.07);
          break;
        }

        case 'cancel': {
          // Soft descending dismiss tone (E5 659Hz -> C5 523Hz, ~80ms)
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();

          osc.type = 'sine';
          osc.frequency.setValueAtTime(659.25, now);
          osc.frequency.exponentialRampToValueAtTime(523.25, now + 0.08);

          gain.gain.setValueAtTime(0.08, now);
          gain.gain.exponentialRampToValueAtTime(0.001, now + 0.085);

          osc.connect(gain);
          gain.connect(ctx.destination);

          osc.start(now);
          osc.stop(now + 0.085);
          break;
        }

        case 'error': {
          // Muted low double alert (~120ms total)
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();

          osc.type = 'triangle';
          osc.frequency.setValueAtTime(329.63, now);

          gain.gain.setValueAtTime(0.12, now);
          gain.gain.exponentialRampToValueAtTime(0.001, now + 0.05);
          gain.gain.setValueAtTime(0.1, now + 0.06);
          gain.gain.exponentialRampToValueAtTime(0.001, now + 0.12);

          osc.connect(gain);
          gain.connect(ctx.destination);

          osc.start(now);
          osc.stop(now + 0.12);
          break;
        }
      }
    } catch {
      // Audio playback errors are non-fatal
    }
  }
}

export const voiceTypingSoundPlayer = new VoiceTypingSoundPlayer();
