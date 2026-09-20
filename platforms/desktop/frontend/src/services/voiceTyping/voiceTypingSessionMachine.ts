import i18next from 'i18next';
import type { VoiceTypingHistoryItem } from '../../stores/voiceTypingHistoryStore';
import type {
  TextReplacementRuleSet,
  VoiceTypingContextPreset,
  VoiceTypingContextRule,
} from '../../types/config';
import type { TranscriptSegment, TranscriptUpdate } from '../../types/transcript';
import { formatCjkTypography } from '../../utils/cjkTypography';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { applyTextReplacements } from '../../utils/textProcessing';
import { normalizeTranscriptUpdate } from '../../utils/transcriptTiming';
import type { ForegroundWindowInfo } from '../tauri/contracts';
import type { TranscriptionService } from '../transcriptionService';
import type { VoiceTypingOverlayPayload } from '../voiceTypingWindowService';
import {
  classifyContextRule,
  DEFAULT_VOICE_TYPING_CONTEXT_RULES,
  getCurrentPlatform,
  shouldStripTrailingPunctuation,
  type VoiceTypingContextState,
} from './voiceTypingContext';
import type {
  VoiceTypingOverlayPresenter,
  VoiceTypingPositionResolver,
} from './voiceTypingOverlayPresenter';
import { voiceTypingSoundPlayer } from './voiceTypingSounds';

const ERROR_VISIBILITY_MS = 2000;
const FLUSH_EVENT_SETTLE_MS = 80;
export const DEFAULT_BATCH_SILENCE_TIMEOUT_MS = 1500;

type SessionState = 'idle' | 'preparing' | 'listening' | 'composing' | 'stopping' | 'error';
type SegmentDropReason = 'stale_session' | 'manual_stop_pending' | 'empty_after_normalize';

interface VoiceTypingSessionMachineOptions {
  transcriptionService: TranscriptionService;
  overlayPresenter: VoiceTypingOverlayPresenter;
  resolveOverlayPosition: VoiceTypingPositionResolver;
  resolveOverlayPositionAfterCommit: VoiceTypingPositionResolver;
  ensureMicrophoneStarted: () => Promise<void>;
  resolveMicrophoneGain: () => number;
  injectText: (text: string) => Promise<void>;
  onRuntimeError?: (error: string) => void;
  isSoundEnabled?: () => boolean;
  isCjkSpacingEnabled?: () => boolean;
  getProcessingMode?: () => 'raw' | 'polish';
  polishText?: (
    text: string,
    context?: VoiceTypingContextState | null,
    onError?: (error: unknown) => void
  ) => Promise<string>;
  onTextCommitted?: (entry: {
    rawText: string;
    polishedText?: string;
    injectedText: string;
    mode: 'raw' | 'polish' | 'translate';
  }) => void;
  translateText?: (
    text: string,
    targetLanguage: string,
    onError?: (error: unknown) => void
  ) => Promise<string>;
  getTargetLanguage?: () => string;
  onTransformFailed?: (entry: { originalText: string; instruction: string }) => void;
  onPolishFailed?: (entry: { originalText: string }) => void;
  getFocusedSelectionText?: () => Promise<string | null>;
  transformText?: (
    selectedText: string,
    instruction: string,
    context?: VoiceTypingContextState | null,
    onError?: (error: unknown) => void
  ) => Promise<string>;
  getTextReplacements?: () => TextReplacementRuleSet[] | undefined;
  getForegroundWindowInfo?: () => Promise<ForegroundWindowInfo | null>;
  getContextPreset?: () => VoiceTypingContextPreset | undefined;
  getContextRules?: () => VoiceTypingContextRule[] | undefined;
  isContextAwarenessEnabled?: () => boolean;
  getRecentHistory?: () => VoiceTypingHistoryItem[];
  silenceTimeoutMs?: number;
}

function delay(ms: number) {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });
}

function normalizeCandidateText(text: string, enableCjkSpacing = true): string {
  const trimmed = (text || '').trim();
  if (!enableCjkSpacing) {
    return trimmed;
  }
  return formatCjkTypography(trimmed);
}

function analyzeCandidateText(text: string, enableCjkSpacing = true) {
  const normalizedText = normalizeCandidateText(text, enableCjkSpacing);
  if (normalizedText.length > 0) {
    return {
      normalizedText,
      hasVisibleText: true,
      dropReason: null,
    };
  }

  return {
    normalizedText,
    hasVisibleText: false,
    dropReason: 'empty_after_normalize',
  } satisfies {
    normalizedText: string;
    hasVisibleText: boolean;
    dropReason: SegmentDropReason | null;
  };
}

function hasVisibleCandidateText(text: string) {
  return normalizeCandidateText(text).length > 0;
}

export class VoiceTypingSessionMachine {
  private sessionState: SessionState = 'idle';
  private startRequestId = 0;
  private activeSessionId: string | null = null;
  private currentSegmentId: string | null = null;
  private currentText = '';
  private accumulatedPolishText: string[] = [];
  private selectionContext: string | null = null;
  private currentContext: VoiceTypingContextState | null = null;
  private isTranslateSession = false;
  private manualStopPending = false;
  private revision = 0;
  private readonly committedSegmentIds = new Set<string>();
  private segmentProcessingChain: Promise<void> = Promise.resolve();
  private silenceCommitTimer: number | null = null;

  constructor(private readonly options: VoiceTypingSessionMachineOptions) {}

  async start(options?: { isTranslate?: boolean }) {
    if (this.isActive()) {
      return;
    }
    this.clearSilenceCommitTimer();

    this.isTranslateSession = Boolean(options?.isTranslate);

    const requestId = ++this.startRequestId;
    const sessionId = `voice-typing-${requestId}`;
    this.sessionState = 'preparing';
    this.activeSessionId = sessionId;
    this.currentSegmentId = null;
    this.currentText = '';
    this.accumulatedPolishText = [];
    this.selectionContext = null;
    this.currentContext = null;
    this.manualStopPending = false;
    this.committedSegmentIds.clear();
    this.segmentProcessingChain = Promise.resolve();
    this.options.overlayPresenter.clearListeningReset();
    this.playSound('start');

    const contextProbePromise = Promise.all([
      this.options.getFocusedSelectionText
        ? this.options.getFocusedSelectionText().catch((err) => {
            logger.debug('[VoiceTypingSessionMachine] Failed to probe selection context', err);
            return null;
          })
        : Promise.resolve(null),
      (this.options.isContextAwarenessEnabled?.() ?? true) && this.options.getForegroundWindowInfo
        ? this.options.getForegroundWindowInfo().catch((err) => {
            logger.debug('[VoiceTypingSessionMachine] Failed to probe foreground window info', err);
            return null;
          })
        : Promise.resolve(null),
    ]);

    const preparingPromise = this.publishOverlay(
      {
        sessionId,
        phase: 'preparing',
        text: '',
      },
      { revealIfHidden: true, reposition: true }
    );

    try {
      const service = this.options.transcriptionService;
      const onUpdate = (update: TranscriptUpdate) => {
        this.enqueueSegmentUpdate(sessionId, requestId, update);
      };
      const onError = (error: string) => {
        if (!this.isCurrentSession(sessionId, requestId)) {
          return;
        }

        logger.error('[VoiceTypingSessionMachine] Voice typing transcription error:', error);
        void this.handleSessionError(sessionId, requestId, error);
      };
      const callbackOptions = {
        callbackOwner: 'voice-typing',
        callbackSessionId: sessionId,
      };
      const startPromise = service.prepareNativeStart(onUpdate, onError, callbackOptions);

      const preparingRevision = await preparingPromise;
      await startPromise;

      const [selResult, infoResult] = await contextProbePromise;
      if (selResult && selResult.trim().length > 0) {
        this.selectionContext = selResult.trim();
        logger.info('[VoiceTypingSessionMachine] Selection context detected', {
          length: this.selectionContext.length,
        });
      }

      if (infoResult && (infoResult.appName || infoResult.windowTitle)) {
        const preset = this.options.getContextPreset?.() ?? 'auto';
        const rules = this.options.getContextRules?.() ?? DEFAULT_VOICE_TYPING_CONTEXT_RULES;
        const rule = classifyContextRule(
          infoResult.appName,
          infoResult.windowTitle,
          rules,
          getCurrentPlatform(),
          preset
        );
        this.currentContext = {
          appName: infoResult.appName,
          windowTitle: infoResult.windowTitle,
          preset,
          mode: rule ? rule.id : preset !== 'auto' && preset !== 'general' ? preset : 'general',
          rule,
        };
        logger.info('[VoiceTypingSessionMachine] Sensed application context', this.currentContext);
      }
      if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
        await Promise.resolve(service.stop()).catch(() => {});
        return;
      }

      await this.options.ensureMicrophoneStarted();

      if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
        await Promise.resolve(service.stop()).catch(() => {});
        return;
      }

      await service.attachPreparedNative({
        sourceKind: 'microphone',
        deviceName: null,
        gain: this.options.resolveMicrophoneGain(),
        callbackOwner: 'voice-typing',
        callbackSessionId: sessionId,
      });

      if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
        await Promise.resolve(service.stop()).catch((stopError) => {
          logger.error(
            '[VoiceTypingSessionMachine] Failed to stop orphaned recognizer:',
            stopError
          );
        });
        return;
      }
      if (this.currentText) {
        this.sessionState = 'composing';
        return;
      }

      this.sessionState = 'listening';
      if (this.revision === preparingRevision) {
        await this.publishOverlay({
          sessionId,
          phase: 'listening',
          text: '',
        });
      }
    } catch (error) {
      const errorMessage = extractErrorMessage(error);
      logger.error('[VoiceTypingSessionMachine] Failed to start voice typing:', error);
      await Promise.resolve(this.options.transcriptionService.stop()).catch((stopError) => {
        logger.error(
          '[VoiceTypingSessionMachine] Failed to roll back recognizer after start failure:',
          stopError
        );
      });

      if (this.activeSessionId === sessionId) {
        await this.handleSessionError(sessionId, requestId, errorMessage);
      }
    }
  }

  async stop() {
    if (!this.isActive() || !this.activeSessionId || this.sessionState === 'stopping') {
      return;
    }
    this.clearSilenceCommitTimer();

    const sessionId = this.activeSessionId;
    this.sessionState = 'stopping';
    this.manualStopPending = true;
    this.options.overlayPresenter.clearListeningReset();

    logger.info('[VoiceTypingSessionMachine] Stop requested', {
      sessionId,
      currentSegmentId: this.currentSegmentId,
      textLength: this.currentText.length,
      revision: this.revision,
    });

    await this.options.transcriptionService.softStop().catch((stopError) => {
      logger.error(
        '[VoiceTypingSessionMachine] Failed to flush recognizer while stopping:',
        stopError
      );
    });

    await delay(FLUSH_EVENT_SETTLE_MS);
    await this.segmentProcessingChain.catch((error) => {
      logger.error(
        '[VoiceTypingSessionMachine] Failed while waiting for queued segment updates:',
        error
      );
    });

    if (!this.isCurrentSession(sessionId)) {
      return;
    }

    if (this.selectionContext) {
      const instruction = this.accumulatedPolishText.join('').trim() || this.currentText.trim();

      if (instruction) {
        await this.publishOverlay({
          sessionId,
          phase: 'polishing',
          text: instruction,
        });

        let transformedText = instruction;
        let transformFailed = false;
        if (this.options.transformText) {
          try {
            transformedText = await this.options.transformText(
              this.selectionContext,
              instruction,
              this.currentContext,
              () => {
                transformFailed = true;
              }
            );
          } catch (err) {
            transformFailed = true;
            logger.warn(
              '[VoiceTypingSessionMachine] Transform failed, fallback to instruction',
              err
            );
          }
        }

        if (transformFailed) {
          if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(this.selectionContext).catch(() => {});
          }
          this.options.onTransformFailed?.({
            originalText: this.selectionContext,
            instruction,
          });
        }
        const finalText = this.formatFinalText(transformedText);
        this.options.onTextCommitted?.({
          rawText: `[Selection Rewrite] ${instruction}`,
          polishedText: finalText,
          injectedText: finalText,
          mode: 'polish',
        });
        try {
          await this.options.injectText(finalText);
          this.playSound('commit');
        } catch (error) {
          if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(finalText).catch(() => {});
          }
          logger.error('[VoiceTypingSessionMachine] Failed to inject transformed text:', error);
          if (this.isCurrentSession(sessionId)) {
            await this.handleSessionError(
              sessionId,
              this.startRequestId,
              extractErrorMessage(error)
            );
          }
          return;
        }
      }

      await this.closeSession(sessionId);
      return;
    }

    if (this.isTranslateSession) {
      const rawFullText = this.accumulatedPolishText.join('').trim() || this.currentText.trim();
      if (rawFullText) {
        const mode = this.options.getProcessingMode?.() || 'raw';
        let intermediateText = rawFullText;

        // 1. Inherit polish logic if polish mode is enabled
        if (mode === 'polish' && this.options.polishText) {
          await this.publishOverlay({
            sessionId,
            phase: 'polishing',
            text: rawFullText,
          });

          try {
            intermediateText = await this.options.polishText(
              rawFullText,
              this.currentContext,
              (err) => {
                logger.warn(
                  '[VoiceTypingSessionMachine] Polish before translate failed, fallback to raw',
                  err
                );
              }
            );
          } catch (err) {
            logger.warn(
              '[VoiceTypingSessionMachine] Polish before translate failed, fallback to raw',
              err
            );
          }
        }

        // 2. Translate step
        await this.publishOverlay({
          sessionId,
          phase: 'translating',
          text: intermediateText,
        });

        let translatedText = intermediateText;
        const targetLanguage = this.options.getTargetLanguage?.() || 'en';
        if (this.options.translateText) {
          try {
            translatedText = await this.options.translateText(
              intermediateText,
              targetLanguage,
              (err) => {
                logger.warn('[VoiceTypingSessionMachine] Translation failed', err);
              }
            );
          } catch (err) {
            logger.warn(
              '[VoiceTypingSessionMachine] Translation failed, using intermediate text',
              err
            );
          }
        }

        const finalText = this.formatFinalText(translatedText);
        this.options.onTextCommitted?.({
          rawText: rawFullText,
          polishedText: mode === 'polish' ? intermediateText : undefined,
          injectedText: finalText,
          mode: 'translate',
        });

        try {
          await this.options.injectText(finalText);
          this.playSound('commit');
        } catch (error) {
          if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(finalText).catch(() => {});
          }
          logger.error('[VoiceTypingSessionMachine] Failed to inject translated text:', error);
          if (this.isCurrentSession(sessionId)) {
            await this.handleSessionError(
              sessionId,
              this.startRequestId,
              extractErrorMessage(error)
            );
          }
          return;
        }
      }

      await this.closeSession(sessionId);
      return;
    }

    const mode = this.options.getProcessingMode?.() || 'raw';
    if (mode === 'polish') {
      const rawFullText = this.accumulatedPolishText.join('').trim() || this.currentText.trim();

      if (rawFullText) {
        await this.publishOverlay({
          sessionId,
          phase: 'polishing',
          text: rawFullText,
        });

        let polishedText = rawFullText;
        let polishFailed = false;
        if (this.options.polishText) {
          try {
            polishedText = await this.options.polishText(rawFullText, this.currentContext, () => {
              polishFailed = true;
            });
          } catch (err) {
            polishFailed = true;
            logger.warn('[VoiceTypingSessionMachine] Polish failed, fallback to raw', err);
          }
        }

        if (polishFailed) {
          if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(rawFullText).catch(() => {});
          }
          this.options.onPolishFailed?.({
            originalText: rawFullText,
          });
        }
        const finalText = this.formatFinalText(polishedText);
        this.options.onTextCommitted?.({
          rawText: rawFullText,
          polishedText,
          injectedText: finalText,
          mode: 'polish',
        });
        try {
          await this.options.injectText(finalText);
          this.playSound('commit');
        } catch (error) {
          if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(finalText).catch(() => {});
          }
          logger.error('[VoiceTypingSessionMachine] Failed to inject polished text:', error);
          if (this.isCurrentSession(sessionId)) {
            await this.handleSessionError(
              sessionId,
              this.startRequestId,
              extractErrorMessage(error)
            );
          }
          return;
        }
      }
    }

    await this.closeSession(sessionId);
  }

  async cancel() {
    if (!this.isActive() || !this.activeSessionId || this.sessionState === 'stopping') {
      return;
    }
    this.clearSilenceCommitTimer();

    const sessionId = this.activeSessionId;
    this.sessionState = 'stopping';
    this.manualStopPending = true;
    this.isTranslateSession = false;
    this.currentText = '';
    this.currentSegmentId = null;
    this.options.overlayPresenter.clearListeningReset();

    logger.info('[VoiceTypingSessionMachine] Cancel requested', {
      sessionId,
      revision: this.revision,
    });

    this.accumulatedPolishText = [];
    this.playSound('cancel');

    await this.options.transcriptionService.softStop().catch((stopError) => {
      logger.error(
        '[VoiceTypingSessionMachine] Failed to stop recognizer while cancelling:',
        stopError
      );
    });

    if (!this.isCurrentSession(sessionId)) {
      return;
    }

    await this.closeSession(sessionId);
  }
  async openQuickRecall(position?: [number, number]) {
    if (this.isActive()) {
      return;
    }
    const requestId = ++this.startRequestId;
    const sessionId = `recall-${requestId}`;
    this.sessionState = 'composing';
    this.activeSessionId = sessionId;
    await this.publishOverlay(
      {
        sessionId,
        phase: 'recall',
        text: '',
      },
      {
        revealIfHidden: true,
        reposition: Boolean(position),
        resolvePosition: position ? async () => position : undefined,
        focus: true,
      }
    );
  }

  isActive() {
    return this.activeSessionId !== null;
  }
  isRecallActive() {
    return this.activeSessionId?.startsWith('recall-') ?? false;
  }

  getLastPosition() {
    return this.options.overlayPresenter.getLastPosition();
  }

  getLastPayload() {
    return this.options.overlayPresenter.getLastPayload();
  }

  resetForTest() {
    this.sessionState = 'idle';
    this.startRequestId = 0;
    this.activeSessionId = null;
    this.currentSegmentId = null;
    this.currentText = '';
    this.accumulatedPolishText = [];
    this.selectionContext = null;
    this.currentContext = null;
    this.manualStopPending = false;
    this.revision = 0;
    this.committedSegmentIds.clear();
    this.segmentProcessingChain = Promise.resolve();
    this.options.overlayPresenter.clearListeningReset();
  }

  private enqueueSegmentUpdate(
    sessionId: string,
    requestId: number,
    update: TranscriptUpdate | TranscriptSegment
  ) {
    const normalizedUpdate = normalizeTranscriptUpdate(update);
    const run = async () => {
      try {
        for (const segment of normalizedUpdate.upsertSegments) {
          await this.handleSegmentUpdate(sessionId, requestId, segment);
        }
      } catch (error) {
        logger.error('[VoiceTypingSessionMachine] Failed to process segment update:', error);
      }
    };

    this.segmentProcessingChain = this.segmentProcessingChain.then(run, run);
  }

  private async handleSegmentUpdate(
    sessionId: string,
    requestId: number,
    segment: TranscriptSegment
  ) {
    if (!this.isCurrentSession(sessionId, requestId)) {
      this.logSegmentDrop('stale_session', {
        sessionId,
        requestId,
        segmentId: segment.id,
        final: segment.isFinal,
        rawTextLength: segment.text.length,
        revision: this.revision,
      });
      return;
    }

    if (this.sessionState === 'error') {
      return;
    }

    if (this.manualStopPending && !segment.isFinal) {
      this.logSegmentDrop('manual_stop_pending', {
        sessionId,
        requestId,
        segmentId: segment.id,
        final: segment.isFinal,
        rawTextLength: segment.text.length,
        currentSegmentId: this.currentSegmentId,
        revision: this.revision,
      });
      return;
    }

    const enableCjkSpacing = this.options.isCjkSpacingEnabled?.() ?? true;
    const {
      normalizedText: text,
      hasVisibleText,
      dropReason,
    } = analyzeCandidateText(segment.text, enableCjkSpacing);

    const mode = this.options.getProcessingMode?.() || 'raw';
    if (this.selectionContext || mode === 'polish' || this.isTranslateSession) {
      if (segment.isFinal) {
        if (text) {
          this.accumulatedPolishText.push(text);
        }
        this.currentText = this.accumulatedPolishText.join('');
        this.currentSegmentId = null;

        const totalText = this.currentText.trim();
        if (totalText && !this.manualStopPending && this.isActive()) {
          this.armSilenceCommitTimer(sessionId, requestId);
        }
      } else {
        if (hasVisibleText) {
          this.clearSilenceCommitTimer();
        }
        this.currentSegmentId = segment.id;
        this.currentText = [...this.accumulatedPolishText, text].join('');
      }

      await this.publishOverlay(
        {
          sessionId,
          phase: 'segment',
          text: this.currentText,
          segmentId: segment.id,
          isFinal: segment.isFinal,
        },
        { revealIfHidden: !this.options.overlayPresenter.isVisible() }
      );
      return;
    }
    const isCurrentSentence =
      this.currentSegmentId === null || this.currentSegmentId === segment.id;
    const hadVisibleCandidate = hasVisibleCandidateText(this.currentText);

    if (this.committedSegmentIds.has(segment.id)) {
      logger.info('[VoiceTypingSessionMachine] Ignored stale segment for committed sentence', {
        sessionId,
        requestId,
        segmentId: segment.id,
        final: segment.isFinal,
        textLength: text.length,
        revision: this.revision,
      });
      return;
    }

    logger.info('[VoiceTypingSessionMachine] Segment update received', {
      sessionId,
      requestId,
      segmentId: segment.id,
      final: segment.isFinal,
      rawTextLength: segment.text.length,
      textLength: text.length,
      currentSegmentId: this.currentSegmentId,
      currentTextLength: this.currentText.length,
      hadVisibleCandidate,
      hasVisibleText,
      phase: this.sessionState,
      revision: this.revision,
    });

    if (!text || !hasVisibleText) {
      this.logSegmentDrop(dropReason ?? 'empty_after_normalize', {
        sessionId,
        requestId,
        segmentId: segment.id,
        final: segment.isFinal,
        rawTextLength: segment.text.length,
        textLength: text.length,
        hadVisibleCandidate,
        keptVisibleCandidate: hadVisibleCandidate && !segment.isFinal && isCurrentSentence,
        currentSegmentId: this.currentSegmentId,
        revision: this.revision,
      });

      if (!segment.isFinal && isCurrentSentence && !this.manualStopPending) {
        if (hadVisibleCandidate) {
          return;
        }

        this.currentSegmentId = null;
        this.currentText = '';

        if (this.sessionState !== 'listening') {
          this.sessionState = 'listening';
          await this.publishOverlay({
            sessionId,
            phase: 'listening',
            text: '',
          });
        }
      }
      return;
    }

    this.currentSegmentId = segment.id;
    this.currentText = text;
    if (!this.manualStopPending) {
      this.sessionState = 'composing';
    }

    await this.publishOverlay(
      {
        sessionId,
        phase: 'segment',
        text,
        segmentId: segment.id,
        isFinal: segment.isFinal,
      },
      {
        revealIfHidden: !this.options.overlayPresenter.isVisible(),
      }
    );

    if (!this.isCurrentSession(sessionId, requestId) || !segment.isFinal) {
      return;
    }

    const commitReason = this.manualStopPending ? 'manual_stop' : 'vad_final';
    await this.commitSegment(sessionId, requestId, segment.id, text, commitReason);
  }

  private async commitSegment(
    sessionId: string,
    requestId: number,
    segmentId: string,
    text: string,
    commitReason: 'manual_stop' | 'vad_final'
  ) {
    if (!this.isCurrentSession(sessionId, requestId) || !text) {
      return;
    }

    if (this.committedSegmentIds.has(segmentId)) {
      logger.info('[VoiceTypingSessionMachine] Ignored duplicate final segment commit', {
        sessionId,
        requestId,
        segmentId,
        commitReason,
        revision: this.revision,
      });
      return;
    }

    this.committedSegmentIds.add(segmentId);
    logger.info('[VoiceTypingSessionMachine] Committing segment', {
      sessionId,
      requestId,
      segmentId,
      commitReason,
      textLength: text.length,
      hadVisibleCandidate: hasVisibleCandidateText(text),
      revision: this.revision,
    });

    const finalText = this.formatFinalText(text);
    this.options.onTextCommitted?.({
      rawText: text,
      injectedText: finalText,
      mode: 'raw',
    });

    try {
      await this.options.injectText(finalText);
      this.playSound('commit');
    } catch (error) {
      if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(finalText).catch(() => {});
      }
      logger.error('[VoiceTypingSessionMachine] Failed to inject dictated text:', error);
      if (this.isCurrentSession(sessionId, requestId)) {
        await this.handleSessionError(sessionId, requestId, extractErrorMessage(error));
      }
      return;
    }

    if (!this.isCurrentSession(sessionId, requestId)) {
      return;
    }

    if (this.currentSegmentId === segmentId) {
      this.currentSegmentId = null;
      this.currentText = '';
    }

    if (this.manualStopPending || this.sessionState === 'stopping') {
      return;
    }

    this.sessionState = 'listening';
    logger.info('[VoiceTypingSessionMachine] Reset overlay to listening after committed segment', {
      sessionId,
      requestId,
      segmentId,
      commitReason,
      textLength: text.length,
      revision: this.revision,
    });
    await this.publishOverlay(
      {
        sessionId,
        phase: 'listening',
        text: '',
      },
      {
        revealIfHidden: !this.options.overlayPresenter.isVisible(),
        reposition: true,
        resolvePosition: this.options.resolveOverlayPositionAfterCommit,
      }
    );
  }

  private playSound(kind: 'start' | 'commit' | 'cancel' | 'error') {
    if (this.options.isSoundEnabled?.() ?? true) {
      voiceTypingSoundPlayer.play(kind);
    }
  }

  private formatFinalText(text: string): string {
    const enableCjkSpacing = this.options.isCjkSpacingEnabled?.() ?? true;
    let withSpacing = normalizeCandidateText(text, enableCjkSpacing);
    const rules = this.options.getContextRules?.() ?? DEFAULT_VOICE_TYPING_CONTEXT_RULES;
    const shouldStripPunct = shouldStripTrailingPunctuation(
      this.currentContext?.rule ?? this.currentContext?.mode,
      rules
    );
    if (shouldStripPunct) {
      withSpacing = withSpacing.replace(/[。.]+$/, '');
    }
    const replacementSets = this.options.getTextReplacements?.();
    return applyTextReplacements(withSpacing, replacementSets);
  }
  private async handleSessionError(sessionId: string, requestId: number, error: string) {
    this.playSound('error');
    this.options.overlayPresenter.clearListeningReset();
    this.clearSilenceCommitTimer();
    this.sessionState = 'error';
    this.manualStopPending = true;
    this.options.onRuntimeError?.(error);
    await this.publishOverlay(
      {
        sessionId,
        phase: 'error',
        text: `${i18next.t('errors.common.operation_failed')}: ${error}`,
      },
      { revealIfHidden: true }
    );

    await this.options.transcriptionService.softStop().catch((stopError) => {
      logger.error('[VoiceTypingSessionMachine] Failed to stop recognizer after error:', stopError);
    });

    if (!this.isCurrentSession(sessionId, requestId)) {
      return;
    }

    await delay(ERROR_VISIBILITY_MS);
    await this.closeSession(sessionId);
  }

  private async publishOverlay(
    payload: Omit<VoiceTypingOverlayPayload, 'revision'>,
    options?: {
      revealIfHidden?: boolean;
      reposition?: boolean;
      resolvePosition?: VoiceTypingPositionResolver;
      focus?: boolean;
    }
  ) {
    const nextPayload: VoiceTypingOverlayPayload = {
      ...payload,
      hasSelection: Boolean(this.selectionContext),
      selectionLength: this.selectionContext?.length,
      contextMode: this.currentContext?.mode,
      contextName: this.currentContext?.rule?.name,
      contextIcon: this.currentContext?.rule?.icon,
      contextColor: this.currentContext?.rule?.badgeColor,
      history:
        payload.history ??
        (payload.phase === 'recall' ? this.options.getRecentHistory?.() : undefined),
      revision: ++this.revision,
    };

    await this.options.overlayPresenter.publish(nextPayload, {
      ...options,
      focus: options?.focus ?? payload.phase === 'recall',
      resolvePosition: options?.resolvePosition ?? this.options.resolveOverlayPosition,
    });
    return nextPayload.revision;
  }

  private async closeSession(sessionId: string) {
    await this.options.overlayPresenter.hide().catch((error) => {
      logger.error('[VoiceTypingSessionMachine] Failed to hide overlay:', error);
    });
    await this.options.overlayPresenter.clearState();
    await Promise.resolve(this.options.transcriptionService.stop()).catch(() => {});
    this.clearSilenceCommitTimer();
    this.finishSession(sessionId);
  }
  private logSegmentDrop(reason: SegmentDropReason, details: Record<string, unknown>) {
    logger.info('[VoiceTypingSessionMachine] Dropped segment update', {
      dropReason: reason,
      phase: this.sessionState,
      manualStopPending: this.manualStopPending,
      ...details,
    });
  }

  private isCurrentSession(sessionId: string, requestId?: number) {
    return (
      this.activeSessionId === sessionId &&
      (requestId === undefined || requestId === this.startRequestId)
    );
  }

  private finishSession(sessionId: string) {
    if (this.activeSessionId !== sessionId) {
      return;
    }

    this.activeSessionId = null;
    this.currentSegmentId = null;
    this.currentText = '';
    this.selectionContext = null;
    this.currentContext = null;
    this.manualStopPending = false;
    this.committedSegmentIds.clear();
    this.segmentProcessingChain = Promise.resolve();
    this.sessionState = 'idle';
  }

  private isSessionStopping() {
    return this.sessionState === 'stopping' || this.sessionState === 'error';
  }

  private clearSilenceCommitTimer() {
    if (this.silenceCommitTimer !== null) {
      window.clearTimeout(this.silenceCommitTimer);
      this.silenceCommitTimer = null;
    }
  }

  private armSilenceCommitTimer(sessionId: string, requestId: number) {
    this.clearSilenceCommitTimer();
    const timeoutMs = this.options.silenceTimeoutMs ?? DEFAULT_BATCH_SILENCE_TIMEOUT_MS;
    this.silenceCommitTimer = window.setTimeout(() => {
      this.silenceCommitTimer = null;
      if (
        this.isCurrentSession(sessionId, requestId) &&
        !this.manualStopPending &&
        this.isActive()
      ) {
        logger.info('[VoiceTypingSessionMachine] Auto-committing due to prolonged VAD silence', {
          sessionId,
          requestId,
          timeoutMs,
          accumulatedSegments: this.accumulatedPolishText.length,
          textLength: this.currentText.length,
        });
        void this.stop();
      }
    }, timeoutMs);
  }
}
