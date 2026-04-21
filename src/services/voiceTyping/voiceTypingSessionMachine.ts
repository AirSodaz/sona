import { TranscriptSegment } from '../../types/transcript';
import i18next from 'i18next';
import { TranscriptionService } from '../transcriptionService';
import {
    VoiceTypingOverlayPayload,
} from '../voiceTypingWindowService';
import { logger } from '../../utils/logger';
import {
    VoiceTypingOverlayPresenter,
    VoiceTypingPositionResolver,
} from './voiceTypingOverlayPresenter';

const LISTENING_RESET_VISIBILITY_MS = 350;
const HOLD_FINAL_SEGMENT_VISIBILITY_MS = 700;
const FLUSH_EVENT_SETTLE_MS = 80;

type VoiceTypingMode = 'hold' | 'toggle';
type SessionState = 'idle' | 'starting' | 'listening' | 'stopping';

interface VoiceTypingSessionMachineOptions {
    transcriptionService: TranscriptionService;
    overlayPresenter: VoiceTypingOverlayPresenter;
    resolveOverlayPosition: VoiceTypingPositionResolver;
    ensureMicrophoneStarted: () => Promise<void>;
    injectText: (text: string) => Promise<void>;
    getVoiceTypingMode: () => VoiceTypingMode;
}

function delay(ms: number) {
    return new Promise<void>((resolve) => {
        setTimeout(resolve, ms);
    });
}

export class VoiceTypingSessionMachine {
    private isListening = false;
    private sessionState: SessionState = 'idle';
    private startRequestId = 0;
    private activeSessionId: string | null = null;
    private activeSegmentId: string | null = null;
    private revision = 0;

    constructor(private readonly options: VoiceTypingSessionMachineOptions) { }

    async start() {
        if (this.isListening) {
            return;
        }

        const requestId = ++this.startRequestId;
        const sessionId = `voice-typing-${requestId}`;
        this.revision = 0;
        this.isListening = true;
        this.sessionState = 'starting';
        this.activeSessionId = sessionId;
        this.activeSegmentId = null;
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
                async (segment) => {
                    await this.handleSegmentUpdate(sessionId, requestId, segment);
                },
                (error) => {
                    if (!this.isCurrentSession(sessionId, requestId)) {
                        return;
                    }

                    logger.error('Voice typing transcription error:', error);
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

            this.sessionState = 'listening';
            if (this.revision === preparingRevision) {
                await this.publishOverlay({
                    sessionId,
                    phase: 'listening',
                    text: '',
                });
            }
        } catch (error) {
            logger.error('Failed to start voice typing:', error);
            await this.options.transcriptionService.softStop().catch((stopError) => {
                logger.error(
                    'Failed to roll back voice typing recognizer after start failure:',
                    stopError
                );
            });

            if (this.activeSessionId === sessionId) {
                await this.options.overlayPresenter.hide();
                await this.options.overlayPresenter.clearState();
                this.finishSession(sessionId);
            }
        }
    }

    async stop() {
        if (!this.isListening || !this.activeSessionId || this.sessionState === 'stopping') {
            return;
        }

        const sessionId = this.activeSessionId;
        const mode = this.options.getVoiceTypingMode();
        this.sessionState = 'stopping';
        this.options.overlayPresenter.clearListeningReset();

        await this.options.transcriptionService.softStop().catch((stopError) => {
            logger.error('Failed to flush voice typing recognizer while stopping:', stopError);
        });

        if (!this.isCurrentSession(sessionId)) {
            return;
        }

        if (!this.options.overlayPresenter.isFinalSegmentVisible(sessionId)) {
            await delay(FLUSH_EVENT_SETTLE_MS);
        }

        if (!this.isCurrentSession(sessionId)) {
            return;
        }

        if (this.options.overlayPresenter.isFinalSegmentVisible(sessionId)) {
            await delay(
                mode === 'hold'
                    ? HOLD_FINAL_SEGMENT_VISIBILITY_MS
                    : LISTENING_RESET_VISIBILITY_MS
            );
        }

        await this.options.overlayPresenter.hide().catch((error) => {
            logger.error('[VoiceTypingSessionMachine] Failed to hide overlay:', error);
        });
        await this.options.overlayPresenter.clearState();
        this.finishSession(sessionId);
    }

    async handleSegmentUpdate(
        sessionId: string,
        requestId: number,
        segment: TranscriptSegment
    ) {
        if (!this.isCurrentSession(sessionId, requestId)) {
            return;
        }

        const text = segment.text.trim();
        const isNewSegment = this.activeSegmentId !== segment.id;
        this.options.overlayPresenter.clearListeningReset();

        if (!text) {
            if (
                !segment.isFinal &&
                this.sessionState === 'listening' &&
                (!this.activeSegmentId || this.activeSegmentId === segment.id)
            ) {
                this.activeSegmentId = null;
                await this.publishOverlay({
                    sessionId,
                    phase: 'listening',
                    text: '',
                });
            }
            return;
        }

        this.activeSegmentId = segment.id;
        const segmentRevision = await this.publishOverlay(
            {
                sessionId,
                phase: 'segment',
                text,
                segmentId: segment.id,
                isFinal: segment.isFinal,
            },
            {
                revealIfHidden: !this.options.overlayPresenter.isVisible(),
                reposition: this.options.overlayPresenter.isVisible() && isNewSegment,
            }
        );

        if (!this.isCurrentSession(sessionId, requestId)) {
            return;
        }

        if (!segment.isFinal) {
            return;
        }

        await this.options.injectText(text);

        if (!this.isCurrentSession(sessionId, requestId)) {
            return;
        }

        if (
            this.sessionState === 'listening' &&
            this.options.getVoiceTypingMode() === 'toggle'
        ) {
            this.options.overlayPresenter.scheduleListeningReset(() => {
                if (
                    !this.isCurrentSession(sessionId, requestId) ||
                    this.sessionState !== 'listening' ||
                    this.activeSegmentId !== segment.id ||
                    this.revision !== segmentRevision
                ) {
                    return;
                }

                this.activeSegmentId = null;
                void this.publishOverlay({
                    sessionId,
                    phase: 'listening',
                    text: '',
                });
            }, LISTENING_RESET_VISIBILITY_MS);
        }
    }

    async handleSessionError(sessionId: string, requestId: number, error: string) {
        this.options.overlayPresenter.clearListeningReset();
        this.sessionState = 'stopping';
        await this.publishOverlay(
            {
                sessionId,
                phase: 'error',
                text: `${i18next.t('errors.common.operation_failed')}: ${error}`,
            },
            { revealIfHidden: true }
        );

        await this.options.transcriptionService.softStop().catch((stopError) => {
            logger.error('Failed to stop voice typing recognizer after error:', stopError);
        });

        if (!this.isCurrentSession(sessionId, requestId)) {
            return;
        }

        await delay(HOLD_FINAL_SEGMENT_VISIBILITY_MS);
        await this.options.overlayPresenter.hide();
        await this.options.overlayPresenter.clearState();
        this.finishSession(sessionId);
    }

    isActive() {
        return this.isListening;
    }

    getLastPosition() {
        return this.options.overlayPresenter.getLastPosition();
    }

    getLastPayload() {
        return this.options.overlayPresenter.getLastPayload();
    }

    resetForTest() {
        this.isListening = false;
        this.sessionState = 'idle';
        this.startRequestId = 0;
        this.activeSessionId = null;
        this.activeSegmentId = null;
        this.revision = 0;
        this.options.overlayPresenter.clearListeningReset();
    }

    private async publishOverlay(
        payload: Omit<VoiceTypingOverlayPayload, 'revision'>,
        options?: { revealIfHidden?: boolean; reposition?: boolean }
    ) {
        const nextPayload: VoiceTypingOverlayPayload = {
            ...payload,
            revision: ++this.revision,
        };

        await this.options.overlayPresenter.publish(nextPayload, {
            ...options,
            resolvePosition: this.options.resolveOverlayPosition,
        });

        return nextPayload.revision;
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
        this.activeSegmentId = null;
        this.revision = 0;
        this.sessionState = 'idle';
        this.isListening = false;
    }

    private isSessionStopping() {
        return this.sessionState === 'stopping';
    }
}
