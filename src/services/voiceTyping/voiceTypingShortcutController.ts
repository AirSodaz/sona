import { isRegistered, register, unregister } from '@tauri-apps/plugin-global-shortcut';
import { useVoiceTypingRuntimeStore } from '../../stores/voiceTypingRuntimeStore';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';

export type VoiceTypingMode = 'hold' | 'toggle';

export interface VoiceTypingShortcutControllerOptions {
  getMode: () => VoiceTypingMode | string;
  isListening: () => boolean;
  startListening: () => Promise<void>;
  stopListening: () => Promise<void>;
}

export class VoiceTypingShortcutController {
  private isShortcutRegistered = false;
  private currentShortcut: string | null = null;

  constructor(private readonly options: VoiceTypingShortcutControllerOptions) {}

  async update(enabled: boolean, shortcut: string): Promise<void> {
    const normalizedShortcut = shortcut.replace(/\s+/g, '');
    logger.info('[VoiceTypingService] updateShortcutRegistration called', {
      enabled,
      shortcut,
      normalizedShortcut,
    });

    try {
      if (this.isShortcutRegistered && this.currentShortcut) {
        const registered = await isRegistered(this.currentShortcut);
        if (registered) {
          await unregister(this.currentShortcut);
        }
        this.isShortcutRegistered = false;
      }

      if (!enabled || !normalizedShortcut) {
        this.currentShortcut = null;
        useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('idle');
        return;
      }

      this.currentShortcut = null;
      useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('idle');

      await register(normalizedShortcut, (event) => {
        logger.info('[VoiceTypingService] Shortcut event triggered', {
          shortcut: event.shortcut,
          state: event.state,
          mode: this.options.getMode(),
          isListening: this.options.isListening(),
        });

        const mode = this.options.getMode();
        if (mode === 'hold') {
          if (event.state === 'Pressed' && !this.options.isListening()) {
            void this.options.startListening();
          } else if (event.state === 'Released' && this.options.isListening()) {
            void this.options.stopListening();
          }
          return;
        }

        if (event.state === 'Pressed') {
          if (this.options.isListening()) {
            void this.options.stopListening();
          } else {
            void this.options.startListening();
          }
        }
      });

      this.isShortcutRegistered = true;
      this.currentShortcut = normalizedShortcut;
      useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
      logger.info('[VoiceTypingService] Successfully registered voice typing shortcut', {
        shortcut: normalizedShortcut,
      });
    } catch (error) {
      logger.error('[VoiceTypingService] Failed to update voice typing shortcut:', error);
      useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus(
        'error',
        extractErrorMessage(error)
      );
    }
  }

  resetForTest(): void {
    this.isShortcutRegistered = false;
    this.currentShortcut = null;
  }
}
