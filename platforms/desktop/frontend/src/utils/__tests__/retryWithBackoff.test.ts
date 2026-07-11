import { describe, expect, it, vi } from 'vitest';
import { retryWithBackoff } from '../retryWithBackoff';

describe('retryWithBackoff', () => {
  it('retries bounded attempts with injected delay until an attempt succeeds', async () => {
    const delay = vi.fn().mockResolvedValue(undefined);
    const onFailedAttempt = vi.fn();
    const run = vi.fn()
      .mockRejectedValueOnce(new Error('first'))
      .mockRejectedValueOnce(new Error('second'))
      .mockResolvedValueOnce('ok');

    await expect(retryWithBackoff({
      attempts: 3,
      delay,
      delayMs: ({ attempt }) => attempt * 10,
      onFailedAttempt,
      run,
    })).resolves.toBe('ok');

    expect(run).toHaveBeenCalledTimes(3);
    expect(delay).toHaveBeenNthCalledWith(1, 10);
    expect(delay).toHaveBeenNthCalledWith(2, 20);
    expect(onFailedAttempt).toHaveBeenNthCalledWith(
      1,
      expect.any(Error),
      expect.objectContaining({ attempt: 1, willRetry: true }),
    );
    expect(onFailedAttempt).toHaveBeenNthCalledWith(
      2,
      expect.any(Error),
      expect.objectContaining({ attempt: 2, willRetry: true }),
    );
  });

  it('throws the last error without delaying after the final attempt', async () => {
    const delay = vi.fn().mockResolvedValue(undefined);
    const finalError = new Error('final');
    const run = vi.fn()
      .mockRejectedValueOnce(new Error('first'))
      .mockRejectedValueOnce(finalError);

    await expect(retryWithBackoff({
      attempts: 2,
      delay,
      delayMs: 50,
      run,
    })).rejects.toBe(finalError);

    expect(delay).toHaveBeenCalledTimes(1);
  });

  it('stops immediately when a failure is not retryable', async () => {
    const delay = vi.fn().mockResolvedValue(undefined);
    const fatalError = new Error('fatal');
    const run = vi.fn().mockRejectedValue(fatalError);

    await expect(retryWithBackoff({
      attempts: 3,
      delay,
      delayMs: 50,
      run,
      shouldRetry: () => false,
    })).rejects.toBe(fatalError);

    expect(run).toHaveBeenCalledTimes(1);
    expect(delay).not.toHaveBeenCalled();
  });

  it('honors abort signals before starting the next attempt', async () => {
    const controller = new AbortController();
    const delay = vi.fn().mockImplementation(async () => {
      controller.abort();
    });
    const run = vi.fn().mockRejectedValue(new Error('temporary'));

    await expect(retryWithBackoff({
      attempts: 3,
      abortError: () => new Error('Download cancelled'),
      delay,
      delayMs: 1,
      run,
      signal: controller.signal,
    })).rejects.toThrow('Download cancelled');

    expect(run).toHaveBeenCalledTimes(1);
  });
});
