import { logger } from '../../utils/logger';
import {
  type VoiceTypingOverlayPayload,
  voiceTypingWindowService,
} from '../voiceTypingWindowService';

export type VoiceTypingPositionResolver = () => Promise<[number, number]>;

interface PublishOptions {
  revealIfHidden?: boolean;
  reposition?: boolean;
  resolvePosition?: VoiceTypingPositionResolver;
  focus?: boolean;
}

export class VoiceTypingOverlayPresenter {
  private overlayVisible = false;
  private lastOverlayPosition: [number, number] | null = null;
  private lastPayload: VoiceTypingOverlayPayload | null = null;

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

    const shouldFocus = options.focus ?? payload.phase === 'recall';

    if (!nextPosition) {
      if (shouldFocus && this.lastOverlayPosition) {
        await voiceTypingWindowService.open(
          this.lastOverlayPosition[0],
          this.lastOverlayPosition[1],
          true
        );
      }
      return;
    }
    if (shouldFocus) {
      await voiceTypingWindowService.open(nextPosition[0], nextPosition[1], true);
    } else {
      await voiceTypingWindowService.open(nextPosition[0], nextPosition[1]);
    }
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
    // No-op kept for lifecycle interface compatibility
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
