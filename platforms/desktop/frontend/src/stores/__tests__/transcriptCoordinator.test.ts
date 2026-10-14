import { beforeEach, describe, expect, it, vi } from 'vitest';
import { logger } from '../../utils/logger';
import { registerActiveRecorderStopHandler, stopActiveRecording } from '../transcriptCoordinator';

vi.mock('../../utils/logger', () => ({
  logger: {
    warn: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  },
}));

describe('transcriptCoordinator stopActiveRecording hardening', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    registerActiveRecorderStopHandler(null);
  });

  it('warns and returns false when no handler is registered', async () => {
    const result = await stopActiveRecording();
    expect(result).toBe(false);
    expect(logger.warn).toHaveBeenCalledWith(expect.stringContaining('no handler registered'));
  });

  it('executes registered handler and returns true on success', async () => {
    const handler = vi.fn().mockResolvedValue(undefined);
    registerActiveRecorderStopHandler(handler);

    const result = await stopActiveRecording({ discard: false });
    expect(result).toBe(true);
    expect(handler).toHaveBeenCalledWith({ discard: false });
  });

  it('clears handler and resets guard even when handler rejects', async () => {
    const error = new Error('teardown failed');
    const handler = vi.fn().mockRejectedValue(error);
    registerActiveRecorderStopHandler(handler);

    const result = await stopActiveRecording();
    expect(result).toBe(false);
    expect(logger.error).toHaveBeenCalledWith(
      expect.stringContaining('Failed to cleanly stop active recording:'),
      error
    );

    // Verify handler was cleared and not called again
    const secondResult = await stopActiveRecording();
    expect(secondResult).toBe(false);
    expect(logger.warn).toHaveBeenCalledWith(expect.stringContaining('no handler registered'));
  });

  it('prevents concurrent overlapping invocations with isStoppingRecording guard', async () => {
    let resolveHandler!: () => void;
    const pendingPromise = new Promise<void>((res) => {
      resolveHandler = res;
    });
    const slowHandler = vi.fn().mockImplementation(() => pendingPromise);

    registerActiveRecorderStopHandler(slowHandler);

    // Start first stop
    const firstStopPromise = stopActiveRecording();

    // Trigger concurrent second stop while first is still awaiting
    const secondResult = await stopActiveRecording();
    expect(secondResult).toBe(false);
    expect(logger.warn).toHaveBeenCalledWith(expect.stringContaining('stop already in-flight'));

    // Finish first stop
    resolveHandler();
    const firstResult = await firstStopPromise;
    expect(firstResult).toBe(true);

    // After completion, guard is reset
    const handlerAfter = vi.fn().mockResolvedValue(undefined);
    registerActiveRecorderStopHandler(handlerAfter);
    const thirdResult = await stopActiveRecording();
    expect(thirdResult).toBe(true);
    expect(handlerAfter).toHaveBeenCalledTimes(1);
  });
});
