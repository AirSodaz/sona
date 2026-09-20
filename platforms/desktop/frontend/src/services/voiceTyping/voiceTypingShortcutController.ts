import { useVoiceTypingRuntimeStore } from '../../stores/voiceTypingRuntimeStore';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { isRegistered, register, unregister } from '../tauri/platform/globalShortcut';

export type VoiceTypingMode = 'hold' | 'toggle';

export interface VoiceTypingShortcutControllerOptions {
  getMode: () => VoiceTypingMode | string;
  isListening: () => boolean;
  startListening: (options?: { isTranslate?: boolean }) => Promise<void>;
  stopListening: () => Promise<void>;
}

export class VoiceTypingShortcutController {
  private isShortcutRegistered = false;
  private currentShortcut: string | null = null;
  private isTranslateShortcutRegistered = false;
  private currentTranslateShortcut: string | null = null;
  private lastToggleTime = 0;
  constructor(private readonly options: VoiceTypingShortcutControllerOptions) {}

  async update(enabled: boolean, shortcut: string, translateShortcut?: string): Promise<void> {
    const normalizedShortcut = shortcut.replace(/\s+/g, '');
    const normalizedTranslateShortcut = (translateShortcut || '').replace(/\s+/g, '');

    logger.info('[VoiceTypingService] updateShortcutRegistration called', {
      enabled,
      shortcut,
      normalizedShortcut,
      translateShortcut,
      normalizedTranslateShortcut,
    });

    try {
      // Unregister previous shortcuts if registered
      if (this.isShortcutRegistered && this.currentShortcut) {
        const registered = await isRegistered(this.currentShortcut);
        if (registered) {
          await unregister(this.currentShortcut);
        }
        this.isShortcutRegistered = false;
      }
      if (this.isTranslateShortcutRegistered && this.currentTranslateShortcut) {
        const registered = await isRegistered(this.currentTranslateShortcut);
        if (registered) {
          await unregister(this.currentTranslateShortcut);
        }
        this.isTranslateShortcutRegistered = false;
      }

      this.currentShortcut = null;
      this.currentTranslateShortcut = null;
      useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('idle');

      if (!enabled) {
        return;
      }

      // 1. Register base shortcut
      if (normalizedShortcut) {
        await register(normalizedShortcut, (event) => {
          this.handleShortcutEvent(event, false);
        });
        this.isShortcutRegistered = true;
        this.currentShortcut = normalizedShortcut;
      }

      // 2. Register translate shortcut if distinct from base
      if (normalizedTranslateShortcut && normalizedTranslateShortcut !== normalizedShortcut) {
        await register(normalizedTranslateShortcut, (event) => {
          this.handleShortcutEvent(event, true);
        });
        this.isTranslateShortcutRegistered = true;
        this.currentTranslateShortcut = normalizedTranslateShortcut;
      }

      useVoiceTypingRuntimeStore.getState().setShortcutRegistrationStatus('ready');
      logger.info('[VoiceTypingService] Successfully registered voice typing shortcuts', {
        shortcut: normalizedShortcut,
        translateShortcut: normalizedTranslateShortcut,
      });
    } catch (error) {
      logger.error('[VoiceTypingService] Failed to update voice typing shortcuts:', error);
      useVoiceTypingRuntimeStore
        .getState()
        .setShortcutRegistrationStatus('error', extractErrorMessage(error));
    }
  }

  private handleShortcutEvent(
    event: { shortcut: string; state: string },
    isTranslate: boolean
  ): void {
    logger.info('[VoiceTypingService] Shortcut event triggered', {
      shortcut: event.shortcut,
      state: event.state,
      isTranslate,
      mode: this.options.getMode(),
      isListening: this.options.isListening(),
    });

    const mode = this.options.getMode();
    if (mode === 'hold') {
      if (event.state === 'Pressed' && !this.options.isListening()) {
        void this.options.startListening({ isTranslate });
      } else if (event.state === 'Released' && this.options.isListening()) {
        void this.options.stopListening();
      }
      return;
    }

    if (event.state === 'Pressed') {
      const now = Date.now();
      if (now - this.lastToggleTime < 400) {
        logger.info(
          '[VoiceTypingShortcutController] Ignoring duplicate toggle shortcut event within debounce window'
        );
        return;
      }
      this.lastToggleTime = now;

      if (this.options.isListening()) {
        void this.options.stopListening();
      } else {
        void this.options.startListening({ isTranslate });
      }
    }
  }

  resetForTest(): void {
    this.isShortcutRegistered = false;
    this.currentShortcut = null;
    this.isTranslateShortcutRegistered = false;
    this.currentTranslateShortcut = null;
    this.lastToggleTime = 0;
  }
}
