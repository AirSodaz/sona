import { buildRecognizerOutputEvent } from '../../tauri/events';
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
    loggerInfo: vi.fn(),
    loggerError: vi.fn(),
    createExternalLiveSource: vi.fn(),
    startExternalLiveTranscription: vi.fn(),
    retireExternalLiveSource: vi.fn(),
    stopLiveTranscription: vi.fn(),
    feedExternalLiveSource: vi.fn(),
  };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.listen,
}));

vi.mock('../../tauri/recognizer', () => ({
  createExternalLiveSource: mocks.createExternalLiveSource,
  startExternalLiveTranscription: mocks.startExternalLiveTranscription,
  retireExternalLiveSource: mocks.retireExternalLiveSource,
  stopLiveTranscription: mocks.stopLiveTranscription,
  feedExternalLiveSource: mocks.feedExternalLiveSource,
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

  it('retires a newly created external source when subscription startup fails', async () => {
    mocks.createExternalLiveSource.mockResolvedValue({ sourceToken: 'source-token' });
    mocks.startExternalLiveTranscription.mockRejectedValue(new Error('start failed'));
    mocks.retireExternalLiveSource.mockResolvedValue(undefined);
    const RecognizerLifecycle = await loadLifecycle();
    const lifecycle = new RecognizerLifecycle('record');
    const onError = vi.fn();

    await expect(
      lifecycle.startExternal({ mode: 'streaming' } as never, onError),
    ).rejects.toThrow('start failed');

    expect(mocks.retireExternalLiveSource).toHaveBeenCalledWith('source-token');
    expect(onError).toHaveBeenCalledWith(expect.stringContaining('start failed'));
    expect(lifecycle.running).toBe(false);
  });

  it('builds the recognizer output event name from the instance id', () => {
    expect(buildRecognizerOutputEvent('voice-typing')).toBe('recognizer-output-voice-typing');
  });

});
