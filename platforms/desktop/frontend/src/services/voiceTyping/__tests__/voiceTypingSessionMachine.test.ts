import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import type { TranscriptSegment } from '../../../types/transcript';
import { VoiceTypingSessionMachine } from '../voiceTypingSessionMachine';

describe('VoiceTypingSessionMachine', () => {
  let machine: VoiceTypingSessionMachine;
  let mockInjectText: Mock;
  let mockTranslateText: Mock;
  let mockOnTextCommitted: Mock;
  let mockTranscriptionService: any;
  let mockOverlayPresenter: any;

  beforeEach(() => {
    mockInjectText = vi.fn().mockResolvedValue(undefined);
    mockTranslateText = vi.fn().mockResolvedValue('slow_fallback_translation');
    mockOnTextCommitted = vi.fn();
    mockTranscriptionService = {
      prepareNativeStart: vi.fn().mockResolvedValue(undefined),
      attachPreparedNative: vi.fn().mockResolvedValue(undefined),
      start: vi.fn().mockResolvedValue(undefined),
      stop: vi.fn().mockResolvedValue(undefined),
      softStop: vi.fn().mockResolvedValue(undefined),
    };
    mockOverlayPresenter = {
      publish: vi.fn().mockResolvedValue(undefined),
      hide: vi.fn().mockResolvedValue(undefined),
      clearState: vi.fn().mockResolvedValue(undefined),
      isVisible: vi.fn().mockReturnValue(true),
      clearListeningReset: vi.fn(),
      setOverlayPosition: vi.fn(),
    };

    machine = new VoiceTypingSessionMachine({
      transcriptionService: mockTranscriptionService,
      overlayPresenter: mockOverlayPresenter,
      resolveOverlayPosition: vi.fn().mockResolvedValue([100, 100]),
      resolveOverlayPositionAfterCommit: vi.fn().mockResolvedValue([100, 100]),
      ensureMicrophoneStarted: vi.fn().mockResolvedValue(undefined),
      resolveMicrophoneGain: () => 1.0,
      injectText: mockInjectText,
      translateText: mockTranslateText,
      getTargetLanguage: () => 'zh',
      onTextCommitted: mockOnTextCommitted,
      getProcessingMode: () => 'raw',
      isSoundEnabled: () => false,
      isCjkSpacingEnabled: () => false,
    });
  });

  it('reuses streaming translation directly on stop without calling slow translateText', async () => {
    await machine.start({ isTranslate: true });

    const segment: TranscriptSegment = {
      id: 'seg-1',
      text: 'Hello world',
      start: 0,
      end: 1,
      isFinal: true,
      translation: '你好世界',
      draftTranslation: undefined,
    };

    // Feed segment update
    (machine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [segment],
    });

    // Wait for segment processing chain
    await (machine as any).segmentProcessingChain;

    // Stop session
    await machine.stop();

    // Assert that the stream translation was reused and injected immediately
    expect(mockInjectText).toHaveBeenCalledWith('你好世界');
    // translateText must NOT be called because streaming translation was reused
    expect(mockTranslateText).not.toHaveBeenCalled();
    expect(mockOnTextCommitted).toHaveBeenCalledWith(
      expect.objectContaining({
        rawText: 'Hello world',
        injectedText: '你好世界',
        mode: 'translate',
      })
    );
  });

  it('falls back to translateText if stream translation was missing', async () => {
    await machine.start({ isTranslate: true });

    const segment: TranscriptSegment = {
      id: 'seg-2',
      text: 'Good morning',
      start: 0,
      end: 1,
      isFinal: true,
      translation: undefined,
    };

    (machine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [segment],
    });

    await (machine as any).segmentProcessingChain;

    await machine.stop();

    // Fallback translation must be called
    expect(mockTranslateText).toHaveBeenCalledWith('Good morning', 'zh', expect.any(Function));
    expect(mockInjectText).toHaveBeenCalledWith('slow_fallback_translation');
  });
});
