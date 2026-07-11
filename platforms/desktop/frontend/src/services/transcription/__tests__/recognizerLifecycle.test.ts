import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
  const listenCallbacks: Record<string, (event: any) => void> = {};

  return {
    listenCallbacks,
    listen: vi.fn(async (eventName: string, callback: (event: any) => void) => {
      listenCallbacks[eventName] = callback;
      return () => {
        delete listenCallbacks[eventName];
      };
    }),
    startRecognizer: vi.fn(async () => undefined),
    stopRecognizer: vi.fn(async () => undefined),
    flushRecognizer: vi.fn(async () => undefined),
    feedAudioChunk: vi.fn(async () => undefined),
    loggerInfo: vi.fn(),
    loggerError: vi.fn(),
  };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.listen,
}));

vi.mock('../../tauri/recognizer', () => ({
  startRecognizer: mocks.startRecognizer,
  stopRecognizer: mocks.stopRecognizer,
  flushRecognizer: mocks.flushRecognizer,
  feedAudioChunk: mocks.feedAudioChunk,
}));

vi.mock('../../../utils/logger', () => ({
  logger: {
    info: mocks.loggerInfo,
    error: mocks.loggerError,
  },
}));

async function loadLifecycle() {
  const module = await import('../recognizerLifecycle');
  module.resetRecognizerLifecycleForTest();
  return module.RecognizerLifecycle;
}

describe('RecognizerLifecycle', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    for (const key of Object.keys(mocks.listenCallbacks)) {
      delete mocks.listenCallbacks[key];
    }
  });

  it('registers one global event bus per instance and dispatches normalized updates', async () => {
    const RecognizerLifecycle = await loadLifecycle();
    const lifecycle = new RecognizerLifecycle('voice-typing');
    const onUpdate = vi.fn();

    lifecycle.registerCallback(onUpdate, vi.fn(), {
      owner: 'hold-session',
      sessionId: 'session-a',
    });
    await lifecycle.ensureGlobalBus();
    await lifecycle.ensureGlobalBus();

    expect(mocks.listen).toHaveBeenCalledTimes(1);
    expect(mocks.listen).toHaveBeenCalledWith(
      'recognizer-output-voice-typing',
      expect.any(Function),
    );

    mocks.listenCallbacks['recognizer-output-voice-typing']?.({
      payload: {
        id: 'seg-1',
        start: 0,
        end: 0.5,
        text: 'hello',
        isFinal: false,
      },
    });

    expect(onUpdate).toHaveBeenCalledWith({
      removeIds: [],
      upsertSegments: [
        expect.objectContaining({
          id: 'seg-1',
          text: 'hello',
          isFinal: false,
        }),
      ],
    });
  });

  it('replaces callback registration and ignores stale callbacks', async () => {
    const RecognizerLifecycle = await loadLifecycle();
    const lifecycle = new RecognizerLifecycle('caption');
    const firstUpdate = vi.fn();
    const secondUpdate = vi.fn();

    lifecycle.registerCallback(firstUpdate, vi.fn(), {
      owner: 'first',
      sessionId: 'session-a',
    });
    lifecycle.registerCallback(secondUpdate, vi.fn(), {
      owner: 'second',
      sessionId: 'session-b',
    });
    await lifecycle.ensureGlobalBus();

    mocks.listenCallbacks['recognizer-output-caption']?.({
      payload: {
        removeIds: [],
        upsertSegments: [{
          id: 'seg-2',
          start: 0,
          end: 1,
          text: 'caption',
          isFinal: true,
        }],
      },
    });

    expect(firstUpdate).not.toHaveBeenCalled();
    expect(secondUpdate).toHaveBeenCalledTimes(1);
    expect(mocks.loggerInfo).toHaveBeenCalledWith(
      expect.stringContaining('[TranscriptionService:caption] Replacing callback registration.'),
    );
  });

  it('coalesces running state across start, feed, flush, and stop', async () => {
    const RecognizerLifecycle = await loadLifecycle();
    const lifecycle = new RecognizerLifecycle('record');

    await lifecycle.start(vi.fn());
    await lifecycle.start(vi.fn());

    expect(mocks.startRecognizer).toHaveBeenCalledTimes(1);
    expect(lifecycle.running).toBe(true);

    const samples = new Int16Array([1, -1, 2]);
    await lifecycle.feedAudioInt16(samples);

    expect(mocks.feedAudioChunk).toHaveBeenCalledTimes(1);
    expect(mocks.feedAudioChunk).toHaveBeenCalledWith(
      'record',
      new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength),
    );

    await lifecycle.flushAndStop();

    expect(mocks.flushRecognizer).toHaveBeenCalledWith('record');
    expect(mocks.stopRecognizer).toHaveBeenCalledWith('record');
    expect(lifecycle.running).toBe(false);

    await lifecycle.stop();
    await lifecycle.feedAudioInt16(new Int16Array([3]));

    expect(mocks.stopRecognizer).toHaveBeenCalledTimes(1);
    expect(mocks.feedAudioChunk).toHaveBeenCalledTimes(1);
  });
});
