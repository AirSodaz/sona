export interface RetryAttemptContext {
  attempt: number;
  attempts: number;
  signal?: AbortSignal;
}

export interface RetryFailedAttemptContext extends RetryAttemptContext {
  willRetry: boolean;
}

interface RetryWithBackoffOptions<T> {
  attempts: number;
  run: (context: RetryAttemptContext) => Promise<T>;
  abortError?: () => Error;
  delay?: (ms: number) => Promise<void>;
  delayMs?: number | ((context: RetryAttemptContext & { error: unknown }) => number);
  onFailedAttempt?: (error: unknown, context: RetryFailedAttemptContext) => void | Promise<void>;
  shouldRetry?: (error: unknown, context: RetryAttemptContext) => boolean | Promise<boolean>;
  signal?: AbortSignal;
}

function defaultDelay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function throwIfAborted(signal: AbortSignal | undefined, abortError: () => Error): void {
  if (signal?.aborted) {
    throw abortError();
  }
}

export async function retryWithBackoff<T>({
  abortError = () => new Error('Operation cancelled'),
  attempts,
  delay = defaultDelay,
  delayMs = 0,
  onFailedAttempt,
  run,
  shouldRetry,
  signal,
}: RetryWithBackoffOptions<T>): Promise<T> {
  if (!Number.isInteger(attempts) || attempts < 1) {
    throw new Error('retryWithBackoff requires at least one attempt.');
  }

  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const context: RetryAttemptContext = { attempt, attempts, signal };
    throwIfAborted(signal, abortError);

    try {
      return await run(context);
    } catch (error) {
      const isLastAttempt = attempt >= attempts;
      const retryable = shouldRetry ? await shouldRetry(error, context) : true;
      const retryAllowed = !isLastAttempt && retryable;
      await onFailedAttempt?.(error, { ...context, willRetry: retryAllowed });

      if (!retryAllowed) {
        throw error;
      }

      const nextDelayMs = typeof delayMs === 'function' ? delayMs({ ...context, error }) : delayMs;
      if (nextDelayMs > 0) {
        await delay(nextDelayMs);
      }
      throwIfAborted(signal, abortError);
    }
  }

  throw new Error('retryWithBackoff exhausted attempts unexpectedly.');
}
