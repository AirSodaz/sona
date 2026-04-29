import { TranscriptSegment, TranscriptUpdate } from '../../types/transcript';
import i18next from 'i18next';
import { TranscriptionService } from '../transcriptionService';
import { VoiceTypingOverlayPayload } from '../voiceTypingWindowService';
import { logger } from '../../utils/logger';
import { normalizeTranscriptUpdate } from '../../utils/transcriptTiming';
import {
    VoiceTypingOverlayPresenter,
    VoiceTypingPositionResolver,
} from './voiceTypingOverlayPresenter';

const ERROR_VISIBILITY_MS = 700;
const FLUSH_EVENT_SETTLE_MS = 80;

type SessionState = 'idle' | 'preparing' | 'listening' | 'composing' | 'stopping' | 'error';
type SegmentDropReason =
    | 'stale_session'
    | 'manual_stop_pending'
    | 'empty_after_normalize'
    | 'tag_only';

interface VoiceTypingSessionMachineOptions {
    transcriptionService: TranscriptionService;
    overlayPresenter: VoiceTypingOverlayPresenter;
    resolveOverlayPosition: VoiceTypingPositionResolver;
    resolveOverlayPositionAfterCommit: VoiceTypingPositionResolver;
    ensureMicrophoneStarted: () => Promise<void>;
    injectText: (text: string) => Promise<void>;
    onRuntimeError?: (error: string) => void;
}

function delay(ms: number) {
    return new Promise<void>((resolve) => {
        setTimeout(resolve, ms);
    });
}

function normalizeCandidateText(text: string) {
    let result = text.trim();

    while (result.startsWith('<|') && result.includes('|>')) {
        const tagEnd = result.indexOf('|>');
        result = result.slice(tagEnd + 2).trim();
    }

    return result;
}

function isControlTagOnlyText(text: string) {
    const trimmed = text.trim();
    if (!trimmed || !trimmed.includes('<|')) {
        return false;
    }

    return trimmed.replace(/(?:<\|[^|]+?\|>\s*)+/g, '').trim().length === 0;
}

function analyzeCandidateText(text: string) {
    const normalizedText = normalizeCandidateText(text);

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
        dropReason: isControlTagOnlyText(text) ? 'tag_only' : 'empty_after_normalize',
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
    private manualStopPending = false;
    // Keep overlay revisions monotonic across the service lifetime so
    // long-lived aux windows never treat a new session as stale state.
    private revision = 0;
    private readonly committedSegmentIds = new Set<string>();
    private segmentProcessingChain: Promise<void> = Promise.resolve();

    constructor(private readonly options: VoiceTypingSessionMachineOptions) { }

    async start() {
        if (this.isActive()) {
            return;
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
            const startPromise = this.options.transcriptionService.start(
                (update) => {
                    this.enqueueSegmentUpdate(sessionId, requestId, update);
                },
                (error) => {
                    if (!this.isCurrentSession(sessionId, requestId)) {
                        return;
                    }

                    logger.error('[VoiceTypingSessionMachine] Voice typing transcription error:', error);
                    void this.handleSessionError(sessionId, requestId, error);
                },
                {
                    callbackOwner: 'voice-typing',
                    callbackSessionId: sessionId,
                }
            );

            const preparingRevision = await preparingPromise;
            await startPromise;

            if (!this.isCurrentSession(sessionId, requestId) || this.isSessionStopping()) {
                return;
            }

            await this.options.ensureMicrophoneStarted();

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
            logger.error('[VoiceTypingSessionMachine] Failed to start voice typing:', error);
            await this.options.transcriptionService.softStop().catch((stopError) => {
                logger.error(
                    '[VoiceTypingSessionMachine] Failed to roll back recognizer after start failure:',
                    stopError
                );
            });

            if (this.activeSessionId === sessionId) {
                await this.closeSession(sessionId);
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

        await this.closeSession(sessionId);
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

        const { normalizedText: text, hasVisibleText, dropReason } = analyzeCandidateText(
            segment.text
        );
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

        try {
            await this.options.injectText(text);
        } catch (error) {
            logger.error('[VoiceTypingSessionMachine] Failed to inject dictated text:', error);
            if (this.isCurrentSession(sessionId, requestId)) {
                await this.handleSessionError(sessionId, requestId, String(error));
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

    private async handleSessionError(sessionId: string, requestId: number, error: string) {
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
            logger.error(
                '[VoiceTypingSessionMachine] Failed to stop recognizer after error:',
                stopError
            );
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
