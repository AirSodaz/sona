import type { LlmConfig } from '../../types/transcript';

export type LlmExecutionPool = 'local_nmt' | 'local_llm' | 'remote_api';
export type LlmJobPriority = 'high' | 'normal';

export const DEFAULT_LLM_POOL_LIMITS: Readonly<Record<LlmExecutionPool, number>> = {
  local_nmt: 1,
  local_llm: 1,
  remote_api: 3,
};

export class LlmJobCancelledError extends Error {
  constructor(message = 'LLM job was cancelled while queued.') {
    super(message);
    this.name = 'LlmJobCancelledError';
  }
}

export function isLlmJobCancelledError(error: unknown): boolean {
  return error instanceof LlmJobCancelledError;
}

export function resolveLlmExecutionPool(
  config: Pick<LlmConfig, 'provider' | 'strategy' | 'model'>
): LlmExecutionPool {
  const provider = (config.provider || '').toLowerCase();
  const strategy = (config.strategy || '').toLowerCase();
  const model = (config.model || '').toLowerCase();

  const isLocalProvider =
    provider === 'local' || provider === 'llama_cpp' || provider === 'local_model';
  const isLocalStrategy = strategy === 'local';
  const isNmtStrategy =
    strategy === 'local_nmt' || strategy === 'local-nmt' || strategy === 'onnx_nmt';
  const isLocal = isLocalProvider || isLocalStrategy || isNmtStrategy;

  const isNmtModel =
    model === 'auto' || model.startsWith('opus-mt-') || model.includes('auto-match nmt');
  const isNmt = isNmtStrategy || (isLocal && isNmtModel);

  if (isNmt) {
    return 'local_nmt';
  }
  if (isLocal) {
    return 'local_llm';
  }
  return 'remote_api';
}

export interface LlmJobScheduleOptions {
  config: Pick<LlmConfig, 'provider' | 'strategy' | 'model'>;
  priority: LlmJobPriority;
  taskId?: string;
  cancelCheck?: () => boolean;
  signal?: AbortSignal;
  onStart?: () => void | Promise<void>;
  onCancelledWhileQueued?: () => void;
}

interface QueuedLlmJob {
  id: number;
  pool: LlmExecutionPool;
  priority: LlmJobPriority;
  cancelCheck?: () => boolean;
  signal?: AbortSignal;
  onCancelledWhileQueued?: () => void;
  start: () => void;
  abort: (reason: Error) => void;
  settled: boolean;
  abortListener?: () => void;
}

/**
 * Global priority-aware concurrency scheduler (`Channel 4`) across all frontend-initiated
 * LLM and Local NMT IPC invocations (`runTranscriptLlmJob`, `polishTranscriptSegments`,
 * `translateTranscriptSegments`, `generateLlmText`).
 */
export class LlmJobScheduler {
  private nextJobSeq = 1;
  private poolLimits: Record<LlmExecutionPool, number> = { ...DEFAULT_LLM_POOL_LIMITS };
  private activeCounts: Record<LlmExecutionPool, number> = {
    local_nmt: 0,
    local_llm: 0,
    remote_api: 0,
  };
  private highQueues: Record<LlmExecutionPool, QueuedLlmJob[]> = {
    local_nmt: [],
    local_llm: [],
    remote_api: [],
  };
  private normalQueues: Record<LlmExecutionPool, QueuedLlmJob[]> = {
    local_nmt: [],
    local_llm: [],
    remote_api: [],
  };

  schedule<T>(options: LlmJobScheduleOptions, runner: () => Promise<T>): Promise<T> {
    if (options.signal?.aborted || options.cancelCheck?.()) {
      options.onCancelledWhileQueued?.();
      return Promise.reject(new LlmJobCancelledError());
    }

    const pool = resolveLlmExecutionPool(options.config);

    return new Promise<T>((resolve, reject) => {
      const execute = async () => {
        try {
          if (options.signal?.aborted || options.cancelCheck?.()) {
            options.onCancelledWhileQueued?.();
            reject(new LlmJobCancelledError());
            return;
          }
          await options.onStart?.();
          const result = await runner();
          resolve(result);
        } catch (error) {
          reject(error);
        } finally {
          this.activeCounts[pool] = Math.max(0, this.activeCounts[pool] - 1);
          this.pump(pool);
        }
      };

      const entry: QueuedLlmJob = {
        id: this.nextJobSeq++,
        pool,
        priority: options.priority,
        cancelCheck: options.cancelCheck,
        signal: options.signal,
        onCancelledWhileQueued: options.onCancelledWhileQueued,
        settled: false,
        start: () => {
          if (entry.settled) return;
          entry.settled = true;
          this.detachAbortListener(entry);
          this.activeCounts[pool] += 1;
          void execute();
        },
        abort: (reason: Error) => {
          if (entry.settled) return;
          entry.settled = true;
          this.detachAbortListener(entry);
          entry.onCancelledWhileQueued?.();
          reject(reason);
        },
      };

      if (options.signal) {
        const onAbort = () => {
          this.removeEntry(pool, options.priority, entry);
          entry.abort(new LlmJobCancelledError());
        };
        entry.abortListener = onAbort;
        options.signal.addEventListener('abort', onAbort, { once: true });
      }

      if (options.priority === 'high') {
        this.highQueues[pool].push(entry);
      } else {
        this.normalQueues[pool].push(entry);
      }

      this.pump(pool);
    });
  }

  /** Re-evaluates all queued jobs and immediately rejects any whose `cancelCheck` is now true. */
  sweepCancelled(): void {
    for (const pool of ['local_nmt', 'local_llm', 'remote_api'] as const) {
      this.sweepQueue(this.highQueues[pool]);
      this.sweepQueue(this.normalQueues[pool]);
    }
  }

  getActiveCount(pool: LlmExecutionPool): number {
    return this.activeCounts[pool];
  }

  getPendingCount(pool: LlmExecutionPool): number {
    return this.highQueues[pool].length + this.normalQueues[pool].length;
  }

  configurePoolLimits(limits: Partial<Record<LlmExecutionPool, number>>): void {
    this.poolLimits = {
      ...this.poolLimits,
      ...limits,
    };
  }

  resetForTests(): void {
    for (const pool of ['local_nmt', 'local_llm', 'remote_api'] as const) {
      for (const item of [...this.highQueues[pool], ...this.normalQueues[pool]]) {
        this.detachAbortListener(item);
      }
      this.highQueues[pool] = [];
      this.normalQueues[pool] = [];
      this.activeCounts[pool] = 0;
    }
    this.poolLimits = { ...DEFAULT_LLM_POOL_LIMITS };
  }

  private pump(pool: LlmExecutionPool): void {
    while (this.activeCounts[pool] < this.poolLimits[pool]) {
      const next = this.dequeueNextValid(pool);
      if (!next) {
        return;
      }
      next.start();
    }
  }

  private dequeueNextValid(pool: LlmExecutionPool): QueuedLlmJob | null {
    const pickFrom = (queue: QueuedLlmJob[]): QueuedLlmJob | null => {
      while (queue.length > 0) {
        const candidate = queue.shift();
        if (!candidate || candidate.settled) continue;
        if (candidate.signal?.aborted || candidate.cancelCheck?.()) {
          candidate.abort(new LlmJobCancelledError());
          continue;
        }
        return candidate;
      }
      return null;
    };

    return pickFrom(this.highQueues[pool]) ?? pickFrom(this.normalQueues[pool]);
  }

  private sweepQueue(queue: QueuedLlmJob[]): void {
    for (let i = queue.length - 1; i >= 0; i -= 1) {
      const candidate = queue[i];
      if (
        !candidate ||
        candidate.settled ||
        candidate.signal?.aborted ||
        candidate.cancelCheck?.()
      ) {
        queue.splice(i, 1);
        if (candidate && !candidate.settled) {
          candidate.abort(new LlmJobCancelledError());
        }
      }
    }
  }

  private removeEntry(
    pool: LlmExecutionPool,
    priority: LlmJobPriority,
    target: QueuedLlmJob
  ): void {
    const queue = priority === 'high' ? this.highQueues[pool] : this.normalQueues[pool];
    const index = queue.indexOf(target);
    if (index >= 0) {
      queue.splice(index, 1);
    }
  }

  private detachAbortListener(entry: QueuedLlmJob): void {
    if (entry.signal && entry.abortListener) {
      entry.signal.removeEventListener('abort', entry.abortListener);
      entry.abortListener = undefined;
    }
  }
}

export const llmJobScheduler = new LlmJobScheduler();
