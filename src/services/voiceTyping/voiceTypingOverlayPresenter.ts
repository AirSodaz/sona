import {
    VoiceTypingOverlayPayload,
    voiceTypingWindowService,
} from '../voiceTypingWindowService';
import { logger } from '../../utils/logger';

export type VoiceTypingPositionResolver = () => Promise<[number, number]>;

interface PublishOptions {
    revealIfHidden?: boolean;
    reposition?: boolean;
    resolvePosition?: VoiceTypingPositionResolver;
}

export class VoiceTypingOverlayPresenter {
    private overlayVisible = false;
    private lastOverlayPosition: [number, number] | null = null;
    private lastPayload: VoiceTypingOverlayPayload | null = null;
    private listeningResetTimer: ReturnType<typeof setTimeout> | null = null;

    private async resolvePosition(resolvePosition?: VoiceTypingPositionResolver) {
        if (resolvePosition) {
            const nextPosition = await resolvePosition();
            this.lastOverlayPosition = nextPosition;
            return nextPosition;
        }

        return this.lastOverlayPosition ?? [0, 0];
    }

    async prepare(position: [number, number]) {
        this.lastOverlayPosition = position;
        await voiceTypingWindowService.prepare(position);
    }

    async publish(payload: VoiceTypingOverlayPayload, options: PublishOptions = {}) {
        this.lastPayload = payload;

        const shouldReveal = options.revealIfHidden ?? false;
        const shouldReposition = options.reposition ?? false;
        let nextPosition: [number, number] | null = null;

        if (shouldReveal) {
            nextPosition = await this.resolvePosition(options.resolvePosition);
            await voiceTypingWindowService.prepare(nextPosition);
        } else if (shouldReposition && this.overlayVisible) {
            nextPosition = await this.resolvePosition(options.resolvePosition);
        }

        logger.info('[VoiceTypingOverlayPresenter] Commit overlay state', {
            sessionId: payload.sessionId,
            revision: payload.revision,
            phase: payload.phase,
            segmentId: payload.segmentId ?? null,
            isFinal: payload.isFinal ?? null,
            textLength: payload.text.length,
            visible: this.overlayVisible,
            revealIfHidden: shouldReveal,
            reposition: shouldReposition,
        });
        await voiceTypingWindowService.sendState(payload);

        if (!nextPosition) {
            return;
        }

        await voiceTypingWindowService.open(nextPosition[0], nextPosition[1]);
        this.overlayVisible = true;
    }

    async hide() {
        this.overlayVisible = false;
        await voiceTypingWindowService.close();
    }

    async clearState() {
        this.lastPayload = null;
        await voiceTypingWindowService.clearState();
    }

    clearListeningReset() {
        if (this.listeningResetTimer) {
            clearTimeout(this.listeningResetTimer);
            this.listeningResetTimer = null;
        }
    }

    scheduleListeningReset(callback: () => void, delayMs: number) {
        this.clearListeningReset();
        this.listeningResetTimer = setTimeout(() => {
            this.listeningResetTimer = null;
            callback();
        }, delayMs);
    }

    isVisible() {
        return this.overlayVisible;
    }

    getLastPosition() {
        return this.lastOverlayPosition;
    }

    isFinalSegmentVisible(sessionId: string) {
        return (
            this.lastPayload?.sessionId === sessionId &&
            this.lastPayload.phase === 'segment' &&
            this.lastPayload.isFinal === true
        );
    }

    getLastPayload() {
        return this.lastPayload;
    }

    resetForTest() {
        this.clearListeningReset();
        this.overlayVisible = false;
        this.lastOverlayPosition = null;
        this.lastPayload = null;
    }
}
