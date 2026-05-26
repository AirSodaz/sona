import { useVoiceTypingRuntimeStore } from '../../stores/voiceTypingRuntimeStore';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import {
  startMicrophoneCapture,
  stopMicrophoneCapture,
} from '../tauri/audio';

const VOICE_TYPING_INSTANCE_ID = 'voice-typing';

export class VoiceTypingMicrophoneRuntime {
  private captureStarted = false;

  async ensureStarted(microphoneId: string | null | undefined): Promise<void> {
    if (this.captureStarted) {
      return;
    }

    try {
      logger.info('[VoiceTypingService] Starting microphone capture for pre-warming...');
      const deviceName = microphoneId && microphoneId !== 'default'
        ? microphoneId
        : null;
      await startMicrophoneCapture({
        deviceName,
        instanceId: VOICE_TYPING_INSTANCE_ID,
      });
      this.captureStarted = true;
    } catch (error) {
      logger.error('[VoiceTypingService] Failed to start microphone capture:', error);
      useVoiceTypingRuntimeStore.getState().setWarmupStatus('error', {
        errorSource: 'microphone',
        errorMessage: extractErrorMessage(error),
      });
    }
  }

  async stop(): Promise<void> {
    if (!this.captureStarted) {
      return;
    }

    try {
      logger.info('[VoiceTypingService] Stopping persistent microphone capture...');
      await stopMicrophoneCapture(VOICE_TYPING_INSTANCE_ID);
      this.captureStarted = false;
    } catch (error) {
      logger.error('[VoiceTypingService] Failed to stop microphone capture:', error);
    }
  }

  resetForTest(): void {
    this.captureStarted = false;
  }
}
