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

  it('does not reuse stream translation and translates full text when multiple segments exist', async () => {
    await machine.start({ isTranslate: true });

    const segment1: TranscriptSegment = {
      id: 'seg-1',
      text: 'Hello ',
      start: 0,
      end: 1,
      isFinal: true,
      translation: '你好',
    };

    const segment2: TranscriptSegment = {
      id: 'seg-2',
      text: 'world',
      start: 1,
      end: 2,
      isFinal: true,
      translation: '世界',
    };

    (machine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [segment1],
    });
    await (machine as any).segmentProcessingChain;

    (machine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [segment2],
    });
    await (machine as any).segmentProcessingChain;

    await machine.stop();

    // When multiple segments exist, stream translation must NOT be reused; full text is translated
    expect(mockTranslateText).toHaveBeenCalledWith('Helloworld', 'zh', expect.any(Function));
    expect(mockInjectText).toHaveBeenCalledWith('slow_fallback_translation');
  });

  it('does not reuse stream translation in polish mode', async () => {
    const mockPolishText = vi.fn().mockResolvedValue('Polished Hello World');
    const polishMachine = new VoiceTypingSessionMachine({
      transcriptionService: mockTranscriptionService,
      overlayPresenter: mockOverlayPresenter,
      resolveOverlayPosition: vi.fn().mockResolvedValue([100, 100]),
      resolveOverlayPositionAfterCommit: vi.fn().mockResolvedValue([100, 100]),
      ensureMicrophoneStarted: vi.fn().mockResolvedValue(undefined),
      resolveMicrophoneGain: () => 1.0,
      injectText: mockInjectText,
      translateText: mockTranslateText,
      polishText: mockPolishText,
      getTargetLanguage: () => 'zh',
      onTextCommitted: mockOnTextCommitted,
      getProcessingMode: () => 'polish',
      isSoundEnabled: () => false,
      isCjkSpacingEnabled: () => false,
    });

    await polishMachine.start({ isTranslate: true });

    const segment: TranscriptSegment = {
      id: 'seg-1',
      text: 'hello world',
      start: 0,
      end: 1,
      isFinal: true,
      translation: '你好世界',
    };

    (polishMachine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [segment],
    });
    await (polishMachine as any).segmentProcessingChain;

    await polishMachine.stop();

    expect(mockPolishText).toHaveBeenCalledWith('hello world', null, expect.any(Function));
    // translateText must be called on the polished text, NOT using the unpolished stream translation
    expect(mockTranslateText).toHaveBeenCalledWith(
      'Polished Hello World',
      'zh',
      expect.any(Function)
    );
    expect(mockInjectText).toHaveBeenCalledWith('slow_fallback_translation');
  });

  it('clears latestStreamDraftTranslation on final segment', async () => {
    await machine.start({ isTranslate: true });

    const partialSegment: TranscriptSegment = {
      id: 'seg-1',
      text: 'Hello',
      start: 0,
      end: 1,
      isFinal: false,
      draftTranslation: '草稿',
    };

    (machine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [partialSegment],
    });
    await (machine as any).segmentProcessingChain;
    expect((machine as any).latestStreamDraftTranslation).toBe('草稿');

    const finalSegment: TranscriptSegment = {
      id: 'seg-1',
      text: 'Hello',
      start: 0,
      end: 1,
      isFinal: true,
      translation: '你好',
      draftTranslation: undefined,
    };

    (machine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [finalSegment],
    });
    await (machine as any).segmentProcessingChain;
    expect((machine as any).latestStreamDraftTranslation).toBeNull();
  });

  it('resets latestStreamDraftTranslation to null on interim partial update without draft', async () => {
    await machine.start({ isTranslate: true });
    interface TestMachineInternals {
      enqueueSegmentUpdate: (
        sessionId: string,
        requestId: number,
        update: { removeIds: string[]; upsertSegments: TranscriptSegment[] }
      ) => void;
      segmentProcessingChain: Promise<void>;
      latestStreamDraftTranslation: string | null;
    }
    const internals = machine as unknown as TestMachineInternals;

    const partialWithDraft: TranscriptSegment = {
      id: 'seg-1',
      text: 'Hello',
      start: 0,
      end: 1,
      isFinal: false,
      draftTranslation: '草稿',
    };

    internals.enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [partialWithDraft],
    });
    await internals.segmentProcessingChain;
    expect(internals.latestStreamDraftTranslation).toBe('草稿');

    const partialWithoutDraft: TranscriptSegment = {
      id: 'seg-1',
      text: 'Hello world',
      start: 0,
      end: 2,
      isFinal: false,
      translation: '你好世界',
      draftTranslation: undefined,
    };

    internals.enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [partialWithoutDraft],
    });
    await internals.segmentProcessingChain;
    expect(internals.latestStreamDraftTranslation).toBeNull();
  });

  it('does not reuse stream translation when no segments have finalized', async () => {
    await machine.start({ isTranslate: true });

    // Only a partial segment is received before stop
    const partialSegment: TranscriptSegment = {
      id: 'seg-1',
      text: 'Good evening',
      start: 0,
      end: 1,
      isFinal: false,
      translation: '晚上', // Partial/incomplete stream translation
    };

    (machine as any).enqueueSegmentUpdate('voice-typing-1', 1, {
      removeIds: [],
      upsertSegments: [partialSegment],
    });
    await (machine as any).segmentProcessingChain;

    await machine.stop();

    // Because the segment was never finalized (accumulatedPolishText.length === 0),
    // the partial stream translation must NOT be reused; full text is translated via translateText
    expect(mockTranslateText).toHaveBeenCalledWith('Good evening', 'zh', expect.any(Function));
    expect(mockInjectText).toHaveBeenCalledWith('slow_fallback_translation');
  });
});
