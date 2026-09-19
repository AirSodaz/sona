import i18next from 'i18next';
import type { TextReplacementRuleSet } from '../../types/config';
import type { TranscriptSegment, TranscriptUpdate } from '../../types/transcript';
import { formatCjkTypography } from '../../utils/cjkTypography';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { applyTextReplacements } from '../../utils/textProcessing';
import { normalizeTranscriptUpdate } from '../../utils/transcriptTiming';
import type { TranscriptionService } from '../transcriptionService';
import type { VoiceTypingOverlayPayload } from '../voiceTypingWindowService';
import type {
  VoiceTypingOverlayPresenter,
  VoiceTypingPositionResolver,
} from './voiceTypingOverlayPresenter';
import { voiceTypingSoundPlayer } from './voiceTypingSounds';

const ERROR_VISIBILITY_MS = 2000;
const FLUSH_EVENT_SETTLE_MS = 80;

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
  polishText?: (text: string) => Promise<string>;
  onTextCommitted?: (entry: {
    rawText: string;
    polishedText?: string;
    injectedText: string;
    mode: 'raw' | 'polish';
  }) => void;
  getFocusedSelectionText?: () => Promise<string | null>;
  transformText?: (selectedText: string, instruction: string) => Promise<string>;
  getTextReplacements?: () => TextReplacementRuleSet[] | undefined;
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
  private manualStopPending = false;
  // long-lived aux windows never treat a new session as stale state.
  private revision = 0;
  private readonly committedSegmentIds = new Set<string>();
  private segmentProcessingChain: Promise<void> = Promise.resolve();

  constructor(private readonly options: VoiceTypingSessionMachineOptions) {}

  async start() {
    if (this.isActive()) {
      return;
    }

    this.accumulatedPolishText = [];
    this.selectionContext = null;
    this.playSound('start');

    if (this.options.getFocusedSelectionText) {
      try {
        const sel = await this.options.getFocusedSelectionText();
        if (sel && sel.trim().length > 0) {
          this.selectionContext = sel.trim();
          logger.info('[VoiceTypingSessionMachine] Selection context detected', {
            length: this.selectionContext.length,
          });
        }
      } catch (err) {
        logger.debug('[VoiceTypingSessionMachine] Failed to probe selection context', err);
      }
    }
    const requestId = ++this.startRequestId;
    const sessionId = `voice-typing-${requestId}`;
    this.sessionState = 'preparing';
    this.activeSessionId = sessionId;
    this.currentSegmentId = null;
    this.currentText = '';
    this.manualStopPending = false;
    this.committedSegmentIds.clear();
    this.segmentProcessingChain = Promise.resolve();
    this.options.overlayPresenter.clearListeningReset();
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

      if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
        return;
      }

      await this.options.ensureMicrophoneStarted();

      if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
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
      await this.options.transcriptionService.softStop().catch((stopError) => {
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
        if (this.options.transformText) {
          try {
            transformedText = await this.options.transformText(this.selectionContext, instruction);
          } catch (err) {
            logger.warn(
              '[VoiceTypingSessionMachine] Transform failed, fallback to instruction',
              err
            );
          }
        }

        const finalText = this.formatFinalText(transformedText);
        try {
          await this.options.injectText(finalText);
          this.playSound('commit');
          this.options.onTextCommitted?.({
            rawText: `[选区修改] ${instruction}`,
            polishedText: finalText,
            injectedText: finalText,
            mode: 'polish',
          });
        } catch (error) {
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
        if (this.options.polishText) {
          try {
            polishedText = await this.options.polishText(rawFullText);
          } catch (err) {
            logger.warn('[VoiceTypingSessionMachine] Polish failed, fallback to raw', err);
          }
        }

        const finalText = this.formatFinalText(polishedText);
        try {
          await this.options.injectText(finalText);
          this.playSound('commit');
          this.options.onTextCommitted?.({
            rawText: rawFullText,
            polishedText,
            injectedText: finalText,
            mode: 'polish',
          });
        } catch (error) {
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

    const sessionId = this.activeSessionId;
    this.sessionState = 'stopping';
    this.manualStopPending = true;
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
      }
    );
  }

  isActive() {
    return this.activeSessionId !== null;
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
    if (this.selectionContext || mode === 'polish') {
      if (segment.isFinal) {
        if (text) {
          this.accumulatedPolishText.push(text);
        }
        this.currentText = this.accumulatedPolishText.join('');
        this.currentSegmentId = null;
      } else {
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

    try {
      await this.options.injectText(finalText);
      this.playSound('commit');
      this.options.onTextCommitted?.({
        rawText: text,
        injectedText: finalText,
        mode: 'raw',
      });
    } catch (error) {
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
    const withSpacing = normalizeCandidateText(text, enableCjkSpacing);
    const replacementSets = this.options.getTextReplacements?.();
    return applyTextReplacements(withSpacing, replacementSets);
  }

  private async handleSessionError(sessionId: string, requestId: number, error: string) {
    this.playSound('error');
    this.options.overlayPresenter.clearListeningReset();
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
    }
  ) {
    const nextPayload: VoiceTypingOverlayPayload = {
      ...payload,
      hasSelection: Boolean(this.selectionContext),
      selectionLength: this.selectionContext?.length,
      revision: ++this.revision,
    };

    await this.options.overlayPresenter.publish(nextPayload, {
      ...options,
      resolvePosition: options?.resolvePosition ?? this.options.resolveOverlayPosition,
    });

    return nextPayload.revision;
  }

  private async closeSession(sessionId: string) {
    await this.options.overlayPresenter.hide().catch((error) => {
      logger.error('[VoiceTypingSessionMachine] Failed to hide overlay:', error);
    });
    await this.options.overlayPresenter.clearState();
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
    this.manualStopPending = false;
    this.committedSegmentIds.clear();
    this.segmentProcessingChain = Promise.resolve();
    this.sessionState = 'idle';
  }

  private isSessionStopping() {
    return this.sessionState === 'stopping' || this.sessionState === 'error';
  }
}
