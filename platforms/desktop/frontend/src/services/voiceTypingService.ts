import i18next from 'i18next';
import { useConfigStore } from '../stores/configStore';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import { useVoiceTypingHistoryStore } from '../stores/voiceTypingHistoryStore';
import { useVoiceTypingRuntimeStore } from '../stores/voiceTypingRuntimeStore';
import type { AppConfig } from '../types/config';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import { isAsrRequestConfigured } from './asrConfigService';
import { TauriEvent } from './tauri/events';
import { listen } from './tauri/platform/events';
import { isRegistered, register, unregister } from './tauri/platform/globalShortcut';
import { currentMonitor, monitorFromPoint } from './tauri/platform/windows';
import { processBatchFile } from './tauri/recognizer';
import {
  getFocusedSelectionText,
  getForegroundWindowInfo,
  getMousePosition,
  getTextCursorPosition,
  injectText,
} from './tauri/system';
import { createTranscriptionService, type TranscriptionService } from './transcriptionService';
import {
  getVoiceTypingShortcutModifiers,
  resolveVoiceTypingAsr,
  resolveVoiceTypingConfigSnapshot,
  resolveVoiceTypingRuntimeChange,
  type VoiceTypingConfigSnapshot,
  type VoiceTypingShortcutModifier,
} from './voiceTyping/voiceTypingConfig';
import { VoiceTypingMicrophoneRuntime } from './voiceTyping/voiceTypingMicrophoneRuntime';
import { VoiceTypingOverlayPresenter } from './voiceTyping/voiceTypingOverlayPresenter';
import {
  polishVoiceTypingText,
  transformSelectedText,
  translateVoiceTypingText,
} from './voiceTyping/voiceTypingPolishService';
import { VoiceTypingSessionMachine } from './voiceTyping/voiceTypingSessionMachine';
import { VoiceTypingShortcutController } from './voiceTyping/voiceTypingShortcutController';
import { voiceTypingSoundPlayer } from './voiceTyping/voiceTypingSounds';
import { VOICE_TYPING_WINDOW_WIDTH } from './voiceTypingWindowService';

const CURSOR_POSITION_OFFSET = 12;
const MOUSE_POSITION_OFFSET = 20;
const POST_COMMIT_CARET_RETRY_DELAYS_MS = [0, 40, 40, 40];
const BOTTOM_CENTER_MARGIN_BOTTOM = 48;

export interface VoiceTypingServicePorts {
  getConfig: () => AppConfig;
  subscribeConfig: typeof useConfigStore.subscribe;
  getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
  getVoiceTypingRuntimeStore: typeof useVoiceTypingRuntimeStore.getState;
  injectText: typeof injectText;
  getTextCursorPosition: typeof getTextCursorPosition;
  getMousePosition: typeof getMousePosition;
  transcriptionService: TranscriptionService;
  listenCancel?: (callback: () => void) => Promise<() => void>;
  currentMonitor?: typeof currentMonitor;
  monitorFromPoint?: typeof monitorFromPoint;
  getFocusedSelectionText?: typeof getFocusedSelectionText;
  getForegroundWindowInfo?: typeof getForegroundWindowInfo;
}

export class VoiceTypingService {
  private initialized = false;

  private lastConfigSnapshot: VoiceTypingConfigSnapshot | null = null;
  private unsubscribe: (() => void) | null = null;
  private cancelUnlisten: (() => void) | null = null;
  private reinjectUnlisten: (() => void) | null = null;
  private quickRecallShortcut: string | null = null;
  private readonly transcriptionService: TranscriptionService;
  private readonly overlayPresenter = new VoiceTypingOverlayPresenter();
  private readonly microphoneRuntime = new VoiceTypingMicrophoneRuntime();
  private readonly sessionMachine: VoiceTypingSessionMachine;
  private readonly shortcutController: VoiceTypingShortcutController;

  constructor(private readonly ports: VoiceTypingServicePorts) {
    this.transcriptionService = ports.transcriptionService;
    this.sessionMachine = new VoiceTypingSessionMachine({
      transcriptionService: this.transcriptionService,
      overlayPresenter: this.overlayPresenter,
      resolveOverlayPosition: () => this.getOverlayPosition(),
      resolveOverlayPositionAfterCommit: () => this.getOverlayPositionAfterCommit(),
      ensureMicrophoneStarted: () => this.ensureMicrophoneStarted(),
      resolveMicrophoneGain: () => this.ports.getConfig().microphoneBoost ?? 1.0,
      injectText: async (text) => {
        const shortcutModifiers = this.getCurrentShortcutModifiers();
        await this.ports.injectText(text, shortcutModifiers);
      },
      onRuntimeError: (error) => {
        this.ports.getVoiceTypingRuntimeStore().reportRuntimeError('session', error);
      },
      isSoundEnabled: () => this.ports.getConfig().voiceTypingSoundEnabled ?? true,
      isCjkSpacingEnabled: () => this.ports.getConfig().voiceTypingCjkSpacingEnabled ?? true,
      getProcessingMode: () => this.ports.getConfig().voiceTypingProcessingMode ?? 'raw',
      polishText: (text, context, onError) => polishVoiceTypingText(text, { context, onError }),
      translateText: (text, targetLanguage, onError) =>
        translateVoiceTypingText(text, targetLanguage, { onError }),
      getTargetLanguage: () =>
        this.ports.getConfig().voiceTypingTargetLanguage ||
        this.ports.getConfig().translationLanguage ||
        'en',
      onPolishFailed: () => {
        const now = Date.now();
        void useTaskLedgerStore.getState().upsertTask({
          id: `voice-typing-polish-fail-${now}`,
          kind: 'llmPolish',
          status: 'failed',
          title: i18next.t('voice_typing.polish_failed', {
            defaultValue: 'Voice typing polish failed',
          }),
          errorMessage: i18next.t('voice_typing.original_saved_to_clipboard', {
            defaultValue: 'Original text was copied to clipboard.',
          }),
          progress: 100,
          createdAt: now,
          updatedAt: now,
          retryable: false,
          cancelable: false,
          recoverable: false,
        });
      },
      onTextCommitted: (entry) => {
        useVoiceTypingHistoryStore.getState().addItem(entry);
      },
      getFocusedSelectionText: () =>
        this.ports.getFocusedSelectionText
          ? this.ports.getFocusedSelectionText()
          : getFocusedSelectionText(),
      transformText: (selectedText, instruction, context, onError) =>
        transformSelectedText(selectedText, instruction, { context, onError }),
      onTransformFailed: () => {
        const now = Date.now();
        void useTaskLedgerStore.getState().upsertTask({
          id: `voice-typing-transform-fail-${now}`,
          kind: 'llmPolish',
          status: 'failed',
          title: i18next.t('voice_typing.selection_rewrite_failed', {
            defaultValue: 'Selection rewrite failed',
          }),
          errorMessage: i18next.t('voice_typing.original_saved_to_clipboard', {
            defaultValue: 'Original text was copied to clipboard.',
          }),
          progress: 100,
          createdAt: now,
          updatedAt: now,
          retryable: false,
          cancelable: false,
          recoverable: false,
        });
      },
      getTextReplacements: () => this.ports.getConfig().textReplacementSets,
      getForegroundWindowInfo: () =>
        this.ports.getForegroundWindowInfo
          ? this.ports.getForegroundWindowInfo()
          : getForegroundWindowInfo(),
      isContextAwarenessEnabled: () =>
        this.ports.getConfig().voiceTypingContextAwarenessEnabled ?? true,
      getContextPreset: () => this.ports.getConfig().voiceTypingContextPreset ?? 'auto',
      getContextRules: () => this.ports.getConfig().voiceTypingContextRules,
      getRecentHistory: () => useVoiceTypingHistoryStore.getState().items.slice(0, 5),
    });
    this.shortcutController = new VoiceTypingShortcutController({
      getMode: () => this.getVoiceTypingMode(),
      isListening: () => this.sessionMachine.isActive(),
      startListening: (options) => this.startListening(options),
      stopListening: () => this.stopListening(),
    });
  }

  public init() {
    if (this.initialized) {
      logger.info('[VoiceTypingService] Already initialized.');
      return;
    }

    this.initialized = true;
    logger.info('[VoiceTypingService] Initializing...');

    const initialConfig = this.ports.getConfig();
    this.lastConfigSnapshot = resolveVoiceTypingConfigSnapshot(initialConfig);

    logger.info('[VoiceTypingService] Initial config', {
      enabled: this.lastConfigSnapshot.enabled,
      shortcut: this.lastConfigSnapshot.shortcut,
      asr: this.lastConfigSnapshot.asrSignature,
      vadModelPath: this.lastConfigSnapshot.vadModelPath,
      microphoneId: this.lastConfigSnapshot.microphoneId,
      keepMicrophoneActive: this.lastConfigSnapshot.keepMicrophoneActive,
      language: this.lastConfigSnapshot.language,
      enableITN: this.lastConfigSnapshot.enableItn,
    });

    this.unsubscribe = this.ports.subscribeConfig((state) => {
      const newConfig = state.config;
      const previousSnapshot =
        this.lastConfigSnapshot ?? resolveVoiceTypingConfigSnapshot(newConfig);
      const nextSnapshot = resolveVoiceTypingConfigSnapshot(newConfig);
      const change = resolveVoiceTypingRuntimeChange(previousSnapshot, nextSnapshot);

      if (!nextSnapshot.enabled) {
        this.ports.getVoiceTypingRuntimeStore().resetRuntimeStatus();
      } else if (change.enabledChanged) {
        this.ports.getVoiceTypingRuntimeStore().clearRuntimeFailure({
          resetShortcutRegistration: true,
          resetWarmup: true,
        });
      } else if (change.runtimeDependencyChanged) {
        this.ports.getVoiceTypingRuntimeStore().clearRuntimeFailure({
          resetShortcutRegistration: change.shortcutChanged,
          resetWarmup:
            change.asrChanged ||
            change.vadModelChanged ||
            change.microphoneChanged ||
            change.keepMicrophoneActiveChanged,
        });
      }

      if (change.enabledChanged || change.shortcutChanged) {
        logger.info('[VoiceTypingService] Shortcut config changed', {
          enabled: nextSnapshot.enabled,
          shortcut: nextSnapshot.shortcut,
        });
        void this.updateShortcutRegistration(
          nextSnapshot.enabled,
          nextSnapshot.shortcut,
          nextSnapshot.translateShortcut
        );
        if (!nextSnapshot.enabled) {
          void this.stopMicrophoneCapture();
        }
      }

      if (
        change.keepMicrophoneActiveChanged &&
        !nextSnapshot.keepMicrophoneActive &&
        !this.sessionMachine.isActive()
      ) {
        void this.stopMicrophoneCapture();
      }

      if (change.enabledChanged || change.quickRecallShortcutChanged) {
        void this.updateQuickRecallShortcutRegistration(
          nextSnapshot.enabled,
          nextSnapshot.quickRecallShortcut
        );
      }

      this.lastConfigSnapshot = nextSnapshot;

      if (nextSnapshot.enabled && (change.configChanged || change.enabledChanged)) {
        void this.syncAndPrepare();
      }
    });

    void this.updateShortcutRegistration(
      this.lastConfigSnapshot.enabled,
      this.lastConfigSnapshot.shortcut,
      this.lastConfigSnapshot.translateShortcut
    );
    void this.updateQuickRecallShortcutRegistration(
      this.lastConfigSnapshot.enabled,
      this.lastConfigSnapshot.quickRecallShortcut
    );
    if (this.lastConfigSnapshot.enabled) {
      void this.syncAndPrepare();
    }
    if (this.ports.listenCancel) {
      void this.ports
        .listenCancel(() => {
          void this.cancelListening();
        })
        .then((unlisten) => {
          this.cancelUnlisten = unlisten;
        });
    }

    void listen<{ text: string }>(TauriEvent.auxWindow.voiceTypingReinject, async (event) => {
      if (event.payload?.text) {
        await this.sessionMachine.cancel();
        await this.delay(80);
        await this.ports.injectText(event.payload.text, this.getCurrentShortcutModifiers());
        voiceTypingSoundPlayer.play('commit');
      }
    })
      .then((unlisten) => {
        this.reinjectUnlisten = unlisten;
      })
      .catch((err) => {
        logger.warn('[VoiceTypingService] Failed to listen to reinject event:', err);
      });
  }

  public destroy() {
    this.initialized = false;
    if (this.unsubscribe) {
      this.unsubscribe();
      this.unsubscribe = null;
    }
    if (this.cancelUnlisten) {
      this.cancelUnlisten();
      this.cancelUnlisten = null;
    }
    if (this.reinjectUnlisten) {
      this.reinjectUnlisten();
      this.reinjectUnlisten = null;
    }
    if (this.quickRecallShortcut) {
      void Promise.resolve(unregister(this.quickRecallShortcut)).catch(() => undefined);
      this.quickRecallShortcut = null;
    }
    this.lastConfigSnapshot = null;
  }

  private async syncAndPrepare() {
    const config = this.ports.getConfig();
    const asr = resolveVoiceTypingAsr(this.ports.getEffectiveConfigSnapshot());
    if (!config.voiceTypingEnabled || !isAsrRequestConfigured(asr)) {
      this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('idle');
      return;
    }

    try {
      this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('preparing');
      logger.info('[VoiceTypingService] Pre-warming transcription model and microphone...');
      this.configureTranscriptionService();
      await this.transcriptionService.prepare();

      const lastPosition = this.sessionMachine.getLastPosition() ?? [0, 0];
      await this.overlayPresenter.prepare(lastPosition);
      if (config.keepMicrophoneActive ?? false) {
        await this.ensureMicrophoneStarted();
      }

      if (this.ports.getVoiceTypingRuntimeStore().warmup === 'error') {
        return;
      }

      this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('ready');

      logger.info('[VoiceTypingService] Model and mic pre-warmed and ready.');
    } catch (error) {
      logger.error('[VoiceTypingService] Failed to pre-warm:', error);
      this.ports.getVoiceTypingRuntimeStore().setWarmupStatus('error', {
        errorSource: 'warmup',
        errorMessage: extractErrorMessage(error),
      });
    }
  }

  private configureTranscriptionService() {
    const asr = resolveVoiceTypingAsr(this.ports.getEffectiveConfigSnapshot());
    if (asr.engine === 'local') {
      this.transcriptionService.setModelPath(asr.modelPath);
    }
    this.transcriptionService.setLanguage(asr.language);
    this.transcriptionService.setEnableITN(asr.enableItn);
  }

  private async ensureMicrophoneStarted() {
    const config = this.ports.getConfig();
    await this.microphoneRuntime.ensureStarted(config.microphoneId);
  }

  private async stopMicrophoneCapture() {
    await this.microphoneRuntime.stop();
  }

  private async updateShortcutRegistration(
    enabled: boolean,
    shortcut: string,
    translateShortcut?: string
  ) {
    await this.shortcutController.update(enabled, shortcut, translateShortcut);
  }

  private async startListening(options?: { isTranslate?: boolean }) {
    this.configureTranscriptionService();
    await this.sessionMachine.start(options);
  }

  private async stopListening() {
    await this.sessionMachine.stop();
    if (!(this.ports.getConfig().keepMicrophoneActive ?? false)) {
      await this.stopMicrophoneCapture();
    }
  }
  public async cancelListening() {
    await this.sessionMachine.cancel();
    if (!(this.ports.getConfig().keepMicrophoneActive ?? false)) {
      await this.stopMicrophoneCapture();
    }
  }

  public async openQuickRecall(): Promise<void> {
    if (this.sessionMachine.isActive()) {
      if (this.sessionMachine.isRecallActive()) {
        await this.sessionMachine.cancel();
      }
      return;
    }
    const position = await this.getQuickRecallOverlayPosition();
    await this.sessionMachine.openQuickRecall(position);
  }

  private async updateQuickRecallShortcutRegistration(
    enabled: boolean,
    shortcut: string
  ): Promise<void> {
    if (this.quickRecallShortcut) {
      try {
        if (await isRegistered(this.quickRecallShortcut)) {
          await unregister(this.quickRecallShortcut);
        }
      } catch (err) {
        logger.warn('[VoiceTypingService] Failed to unregister quick recall shortcut:', err);
      }
      this.quickRecallShortcut = null;
    }

    if (!enabled) {
      return;
    }

    const normalized = shortcut.replace(/\s+/g, '');
    try {
      await register(normalized, (event) => {
        if (event.state === 'Pressed') {
          void this.openQuickRecall();
        }
      });
      this.quickRecallShortcut = normalized;
      logger.info('[VoiceTypingService] Registered quick recall shortcut', {
        shortcut: normalized,
      });
    } catch (err) {
      logger.warn('[VoiceTypingService] Failed to register quick recall shortcut:', err);
    }
  }

  private getVoiceTypingMode() {
    return this.ports.getConfig().voiceTypingMode || 'hold';
  }

  private getCurrentShortcutModifiers(): VoiceTypingShortcutModifier[] {
    const shortcut = this.ports.getConfig().voiceTypingShortcut ?? 'Alt+V';
    return getVoiceTypingShortcutModifiers(shortcut);
  }

  private normalizeOverlayPosition(cursorPosition: [number, number]): [number, number] {
    const marginCompensation = 4;
    return [
      cursorPosition[0] - marginCompensation,
      cursorPosition[1] + CURSOR_POSITION_OFFSET - marginCompensation,
    ];
  }

  private async delay(ms: number) {
    await new Promise<void>((resolve) => {
      setTimeout(resolve, ms);
    });
  }

  private async tryGetTextCursorOverlayPosition() {
    try {
      const cursorPosition = await this.ports.getTextCursorPosition();
      if (!cursorPosition) {
        return null;
      }

      return this.normalizeOverlayPosition(cursorPosition);
    } catch (error) {
      logger.debug(
        '[VoiceTypingService] Failed to get text cursor position, falling back to mouse.',
        error
      );
      return null;
    }
  }
  private async getBottomCenterOverlayPosition(): Promise<[number, number]> {
    try {
      const [mouseX, mouseY] = await this.ports.getMousePosition();
      const getMonitor = this.ports.monitorFromPoint ?? monitorFromPoint;
      const getCurrent = this.ports.currentMonitor ?? currentMonitor;

      let monitor = await getMonitor(mouseX, mouseY).catch(() => null);
      if (!monitor) {
        monitor = await getCurrent().catch(() => null);
      }

      if (monitor) {
        const scale = monitor.scaleFactor || 1;
        const workX = monitor.workArea?.position?.x ?? monitor.position?.x ?? 0;
        const workY = monitor.workArea?.position?.y ?? monitor.position?.y ?? 0;
        const workWidth = monitor.workArea?.size?.width ?? monitor.size?.width ?? 1920;
        const workHeight = monitor.workArea?.size?.height ?? monitor.size?.height ?? 1080;

        const windowPhysicalWidth = Math.round(VOICE_TYPING_WINDOW_WIDTH * scale);
        const windowPhysicalBottomMargin = Math.round((BOTTOM_CENTER_MARGIN_BOTTOM + 40) * scale);

        const targetX = Math.round(workX + (workWidth - windowPhysicalWidth) / 2);
        const targetY = Math.round(workY + workHeight - windowPhysicalBottomMargin);

        return [targetX, targetY];
      }
    } catch (error) {
      logger.debug('[VoiceTypingService] Failed to calculate bottom center position', error);
    }

    return [
      typeof window !== 'undefined' && window.innerWidth
        ? Math.round((window.innerWidth - VOICE_TYPING_WINDOW_WIDTH) / 2)
        : 500,
      800,
    ];
  }

  private async getOverlayPosition(): Promise<[number, number]> {
    const placement = this.ports.getConfig().voiceTypingPlacement ?? 'caret';
    if (placement === 'bottom_center') {
      return await this.getBottomCenterOverlayPosition();
    }

    const cursorPosition = await this.tryGetTextCursorOverlayPosition();
    if (cursorPosition) {
      return cursorPosition;
    }

    const lastPosition = this.sessionMachine.getLastPosition();
    if (lastPosition) {
      return lastPosition;
    }

    const [x, y] = await this.ports.getMousePosition();
    return [x - 4, y + MOUSE_POSITION_OFFSET - 4];
  }

  private async getQuickRecallOverlayPosition(): Promise<[number, number]> {
    const placement = this.ports.getConfig().voiceTypingPlacement ?? 'caret';
    const ESTIMATED_RECALL_HEIGHT = 280;

    if (placement === 'bottom_center') {
      try {
        const [mouseX, mouseY] = await this.ports.getMousePosition();
        const getMonitor = this.ports.monitorFromPoint ?? monitorFromPoint;
        const getCurrent = this.ports.currentMonitor ?? currentMonitor;

        let monitor = await getMonitor(mouseX, mouseY).catch(() => null);
        if (!monitor) {
          monitor = await getCurrent().catch(() => null);
        }

        if (monitor) {
          const scale = monitor.scaleFactor || 1;
          const workX = monitor.workArea?.position?.x ?? monitor.position?.x ?? 0;
          const workY = monitor.workArea?.position?.y ?? monitor.position?.y ?? 0;
          const workWidth = monitor.workArea?.size?.width ?? monitor.size?.width ?? 1920;
          const workHeight = monitor.workArea?.size?.height ?? monitor.size?.height ?? 1080;

          const windowPhysicalWidth = Math.round(VOICE_TYPING_WINDOW_WIDTH * scale);
          const windowPhysicalBottomMargin = Math.round(
            (BOTTOM_CENTER_MARGIN_BOTTOM + ESTIMATED_RECALL_HEIGHT) * scale
          );

          const targetX = Math.round(workX + (workWidth - windowPhysicalWidth) / 2);
          const targetY = Math.max(
            workY + 16,
            Math.round(workY + workHeight - windowPhysicalBottomMargin)
          );

          return [targetX, targetY];
        }
      } catch (error) {
        logger.debug(
          '[VoiceTypingService] Failed to calculate quick recall bottom center position',
          error
        );
      }
    }

    const cursorPosition = await this.tryGetTextCursorOverlayPosition();
    const anchor = cursorPosition ?? (await this.ports.getMousePosition());
    const [anchorX, anchorY] = anchor;

    try {
      const getMonitor = this.ports.monitorFromPoint ?? monitorFromPoint;
      const getCurrent = this.ports.currentMonitor ?? currentMonitor;
      let monitor = await getMonitor(anchorX, anchorY).catch(() => null);
      if (!monitor) {
        monitor = await getCurrent().catch(() => null);
      }

      if (monitor) {
        const scale = monitor.scaleFactor || 1;
        const workY = monitor.workArea?.position?.y ?? monitor.position?.y ?? 0;
        const workHeight = monitor.workArea?.size?.height ?? monitor.size?.height ?? 1080;
        const physicalEstimatedHeight = Math.round(ESTIMATED_RECALL_HEIGHT * scale);
        const physicalBottomMargin = Math.round(16 * scale);
        const maxBottom = workY + workHeight - physicalBottomMargin;

        const normalY = anchorY + (cursorPosition ? 0 : MOUSE_POSITION_OFFSET);
        if (normalY + physicalEstimatedHeight > maxBottom) {
          const flippedY = anchorY - physicalEstimatedHeight - Math.round(12 * scale);
          return [anchorX - 4, Math.max(workY + physicalBottomMargin, flippedY)];
        }
        return [anchorX - 4, normalY];
      }
    } catch (error) {
      logger.debug('[VoiceTypingService] Failed to calculate quick recall cursor position', error);
    }

    return await this.getOverlayPosition();
  }

  private async getOverlayPositionAfterCommit(): Promise<[number, number]> {
    const placement = this.ports.getConfig().voiceTypingPlacement ?? 'caret';
    if (placement === 'bottom_center') {
      const previousPosition = this.sessionMachine.getLastPosition();
      if (previousPosition) {
        return previousPosition;
      }
      return await this.getBottomCenterOverlayPosition();
    }

    const previousPosition = this.sessionMachine.getLastPosition();
    let latestCursorPosition: [number, number] | null = null;
    for (let attempt = 0; attempt < POST_COMMIT_CARET_RETRY_DELAYS_MS.length; attempt += 1) {
      const retryDelay = POST_COMMIT_CARET_RETRY_DELAYS_MS[attempt];
      if (retryDelay > 0) {
        await this.delay(retryDelay);
      }

      const cursorPosition = await this.tryGetTextCursorOverlayPosition();
      const moved =
        !!cursorPosition &&
        (!previousPosition ||
          cursorPosition[0] !== previousPosition[0] ||
          cursorPosition[1] !== previousPosition[1]);

      logger.info('[VoiceTypingService] Post-commit caret probe', {
        attempt: attempt + 1,
        retryDelay,
        previousPosition,
        nextPosition: cursorPosition,
        repositionAttempt: true,
        repositionMoved: moved,
      });

      if (!cursorPosition) {
        continue;
      }

      latestCursorPosition = cursorPosition;
      if (moved) {
        return cursorPosition;
      }
    }

    if (previousPosition) {
      logger.info('[VoiceTypingService] Falling back to last overlay position after commit', {
        previousPosition,
        latestCursorPosition,
        repositionAttempt: true,
        repositionMoved: false,
      });
      return previousPosition;
    }

    if (latestCursorPosition) {
      return latestCursorPosition;
    }

    return await this.getOverlayPosition();
  }

  async retryWarmup() {
    if (!this.initialized) {
      this.init();
      return;
    }

    this.ports.getVoiceTypingRuntimeStore().clearRuntimeFailure({
      resetWarmup: true,
    });
    await this.stopMicrophoneCapture();
    await this.syncAndPrepare();
  }

  resetForTest() {
    this.initialized = false;
    this.lastConfigSnapshot = null;
    this.quickRecallShortcut = null;
    if (this.reinjectUnlisten) {
      this.reinjectUnlisten();
      this.reinjectUnlisten = null;
    }
    this.shortcutController.resetForTest();
    this.overlayPresenter.resetForTest();
    this.sessionMachine.resetForTest();
    this.ports.getVoiceTypingRuntimeStore().resetRuntimeStatus();
  }
}

export function createVoiceTypingService(ports: VoiceTypingServicePorts): VoiceTypingService {
  return new VoiceTypingService(ports);
}

const voiceTypingTranscriptionService = createTranscriptionService('voice-typing', {
  getEffectiveConfigSnapshot,
  processBatchFile,
});

export const voiceTypingService = createVoiceTypingService({
  getConfig: () => useConfigStore.getState().config,
  subscribeConfig: useConfigStore.subscribe,
  getEffectiveConfigSnapshot,
  getVoiceTypingRuntimeStore: useVoiceTypingRuntimeStore.getState,
  injectText,
  getTextCursorPosition,
  getMousePosition,
  transcriptionService: voiceTypingTranscriptionService,
  currentMonitor,
  monitorFromPoint,
  getFocusedSelectionText,
  getForegroundWindowInfo,
  listenCancel: async (callback) => {
    try {
      return await listen(TauriEvent.auxWindow.voiceTypingCancel, callback);
    } catch {
      return () => undefined;
    }
  },
});
