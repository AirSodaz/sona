import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppConfig } from '../../../types/config';
import type { TranscriptSegment } from '../../../types/transcript';
import { StreamingTranslationScheduler } from '../streamingTranslationScheduler';

const mockConfig: AppConfig = {
  general: { appLanguage: 'en', runAtStartup: false, autoCheckUpdates: false },
  appearance: { theme: 'system', fontSize: 'medium' },
  audio: { defaultMic: '', sampleRate: 16000, bufferSize: 1024 },
  asr: {
    selectedModel: 'base',
    selections: {
      realtime: { engine: 'local', modelPath: 'base' },
      batch: { engine: 'local', modelPath: 'base' },
    },
    localModels: [],
  },
  models: { directory: '' },
  shortcuts: {},
  llmSettings: {
    selections: {
      defaultModelId: 'gpt-4o',
      translationModelId: 'gpt-4o',
    },
    models: [
      {
        id: 'gpt-4o',
        name: 'GPT-4o',
        provider: 'openai',
        model: 'gpt-4o',
        enabled: true,
        apiKey: 'test-key',
      },
    ],
  },
  translationLanguage: 'zh-CN',
} as unknown as AppConfig;

function createSegment(id: string, text: string, translation?: string): TranscriptSegment {
  return {
    id,
    start: 0,
    end: 1,
    text,
    translation,
    isFinal: true,
  };
}

describe('StreamingTranslationScheduler', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('batches segments up to batchSize (6) and dispatches to translationService', async () => {
    const mockTranslate = vi
      .fn()
      .mockImplementation(async (_cfg, segments: TranscriptSegment[]) => {
        return segments.map((s) => ({ id: s.id, translation: `Trans: ${s.text}` }));
      });
    const onBatchTranslated = vi.fn();

    const scheduler = new StreamingTranslationScheduler({
      config: mockConfig,
      targetLanguage: 'zh-CN',
      batchSize: 6,
      maxWaitMs: 2500,
      maxConcurrency: 2,
      onBatchTranslated,
      isCancelRequested: () => false,
      translationService: { translateSegmentsWithConfig: mockTranslate },
    });

    for (let i = 1; i <= 5; i++) {
      scheduler.push(createSegment(`seg-${i}`, `Text ${i}`));
    }

    expect(mockTranslate).not.toHaveBeenCalled();
    expect(scheduler.queueLength).toBe(5);

    // 6th segment triggers flush immediately
    scheduler.push(createSegment('seg-6', 'Text 6'));
    expect(mockTranslate).toHaveBeenCalledTimes(1);
    expect(mockTranslate).toHaveBeenCalledWith(
      expect.objectContaining({ translationLanguage: 'zh-CN' }),
      expect.arrayContaining([
        expect.objectContaining({ id: 'seg-1' }),
        expect.objectContaining({ id: 'seg-6' }),
      ])
    );

    await vi.runAllTimersAsync();
    await scheduler.drain();

    expect(onBatchTranslated).toHaveBeenCalledTimes(1);
    const translations = scheduler.getTranslations();
    expect(translations.size).toBe(6);
    expect(translations.get('seg-1')).toBe('Trans: Text 1');
    expect(translations.get('seg-6')).toBe('Trans: Text 6');
  });

  it('flushes trailing segments on maxWaitMs timeout when queue length is below batchSize', async () => {
    const mockTranslate = vi
      .fn()
      .mockImplementation(async (_cfg, segments: TranscriptSegment[]) => {
        return segments.map((s) => ({ id: s.id, translation: `Trans: ${s.text}` }));
      });

    const scheduler = new StreamingTranslationScheduler({
      config: mockConfig,
      targetLanguage: 'zh-CN',
      batchSize: 6,
      maxWaitMs: 2500,
      onBatchTranslated: vi.fn(),
      isCancelRequested: () => false,
      translationService: { translateSegmentsWithConfig: mockTranslate },
    });

    scheduler.push(createSegment('seg-1', 'Trailing 1'));
    scheduler.push(createSegment('seg-2', 'Trailing 2'));

    expect(mockTranslate).not.toHaveBeenCalled();

    // Advance timer past maxWaitMs (2500ms)
    await vi.advanceTimersByTimeAsync(2500);

    expect(mockTranslate).toHaveBeenCalledTimes(1);
    expect(mockTranslate).toHaveBeenCalledWith(
      expect.anything(),
      expect.arrayContaining([
        expect.objectContaining({ id: 'seg-1' }),
        expect.objectContaining({ id: 'seg-2' }),
      ])
    );
  });

  it('deduplicates segments re-pushed with identical IDs', async () => {
    const mockTranslate = vi
      .fn()
      .mockImplementation(async (_cfg, segments: TranscriptSegment[]) => {
        return segments.map((s) => ({ id: s.id, translation: `Trans: ${s.text}` }));
      });

    const scheduler = new StreamingTranslationScheduler({
      config: mockConfig,
      targetLanguage: 'zh-CN',
      batchSize: 6,
      onBatchTranslated: vi.fn(),
      isCancelRequested: () => false,
      translationService: { translateSegmentsWithConfig: mockTranslate },
    });

    scheduler.push(createSegment('seg-1', 'Hello'));
    scheduler.push(createSegment('seg-1', 'Hello')); // duplicate
    scheduler.push(createSegment('seg-1', 'Hello Changed')); // duplicate id

    expect(scheduler.queueLength).toBe(1);

    await scheduler.drain();
    expect(mockTranslate).toHaveBeenCalledTimes(1);
    expect(mockTranslate.mock.calls[0][1]).toHaveLength(1);
  });

  it('enforces maxConcurrency (capping active in-flight requests at 2)', async () => {
    const resolvers: Array<() => void> = [];
    const mockTranslate = vi.fn().mockImplementation(() => {
      const { promise, resolve } =
        Promise.withResolvers<Array<{ id: string; translation: string }>>();
      resolvers.push(() => resolve([{ id: 'seg', translation: 'done' }]));
      return promise;
    });

    const scheduler = new StreamingTranslationScheduler({
      config: mockConfig,
      targetLanguage: 'zh-CN',
      batchSize: 2,
      maxConcurrency: 2,
      onBatchTranslated: vi.fn(),
      isCancelRequested: () => false,
      translationService: { translateSegmentsWithConfig: mockTranslate },
    });

    // Push 6 segments -> 3 batches of 2
    for (let i = 1; i <= 6; i++) {
      scheduler.push(createSegment(`seg-${i}`, `Text ${i}`));
    }

    // Only 2 batches should be in flight concurrently
    expect(mockTranslate).toHaveBeenCalledTimes(2);
    expect(scheduler.activeConcurrencyCount).toBe(2);
    expect(scheduler.queueLength).toBe(2); // 3rd batch waiting in queue

    // Resolve first batch
    resolvers[0]();
    await vi.advanceTimersByTimeAsync(0);

    // Now 3rd batch should be launched
    expect(mockTranslate).toHaveBeenCalledTimes(3);

    // Resolve remaining batches
    resolvers[1]();
    resolvers[2]();
    await scheduler.drain();

    expect(scheduler.activeConcurrencyCount).toBe(0);
    expect(scheduler.queueLength).toBe(0);
  });

  it('trips circuit breaker after 3 consecutive failures without rejecting drain()', async () => {
    let callCount = 0;
    const mockTranslate = vi.fn().mockImplementation(async () => {
      callCount += 1;
      throw new Error(`Translation failure ${callCount}`);
    });
    const onError = vi.fn();

    const scheduler = new StreamingTranslationScheduler({
      config: mockConfig,
      targetLanguage: 'zh-CN',
      batchSize: 2,
      maxConcurrency: 1, // sequential for clear failure progression
      onError,
      onBatchTranslated: vi.fn(),
      isCancelRequested: () => false,
      translationService: { translateSegmentsWithConfig: mockTranslate },
    });

    // Push 10 segments (5 batches of 2)
    for (let i = 1; i <= 10; i++) {
      scheduler.push(createSegment(`seg-${i}`, `Text ${i}`));
    }

    // Await drain - must not throw/reject!
    await scheduler.drain();

    expect(scheduler.isCircuitBroken).toBe(true);
    // After 3 errors, circuit breaker trips and stops launching further batches
    expect(callCount).toBe(3);
    expect(onError).toHaveBeenCalledTimes(3);

    // Pushing after circuit broken should be ignored
    scheduler.push(createSegment('seg-11', 'Text 11'));
    expect(mockTranslate).toHaveBeenCalledTimes(3);
  });

  it('cancel() stops pending timers and clears queues', async () => {
    const mockTranslate = vi.fn().mockResolvedValue([]);

    const scheduler = new StreamingTranslationScheduler({
      config: mockConfig,
      targetLanguage: 'zh-CN',
      batchSize: 6,
      maxWaitMs: 2500,
      onBatchTranslated: vi.fn(),
      isCancelRequested: () => false,
      translationService: { translateSegmentsWithConfig: mockTranslate },
    });

    scheduler.push(createSegment('seg-1', 'Text 1'));
    expect(scheduler.queueLength).toBe(1);

    scheduler.cancel();
    expect(scheduler.queueLength).toBe(0);

    await vi.advanceTimersByTimeAsync(3000);
    expect(mockTranslate).not.toHaveBeenCalled();
  });
});
