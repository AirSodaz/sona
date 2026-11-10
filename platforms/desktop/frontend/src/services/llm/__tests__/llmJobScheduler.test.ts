import { describe, expect, it, vi } from 'vitest';
import { LlmJobCancelledError, LlmJobScheduler, resolveLlmExecutionPool } from '../llmJobScheduler';

describe('resolveLlmExecutionPool', () => {
  it('routes explicit NMT strategies and local auto/opus-mt models to local_nmt', () => {
    expect(resolveLlmExecutionPool({ provider: 'local', strategy: 'local_nmt', model: 'm' })).toBe(
      'local_nmt'
    );
    expect(
      resolveLlmExecutionPool({ provider: 'local', strategy: 'local-nmt' as never, model: 'm' })
    ).toBe('local_nmt');
    expect(
      resolveLlmExecutionPool({ provider: 'local', strategy: 'onnx_nmt' as never, model: 'm' })
    ).toBe('local_nmt');
    expect(resolveLlmExecutionPool({ provider: 'local', model: 'auto' })).toBe('local_nmt');
    expect(resolveLlmExecutionPool({ provider: 'local', model: 'opus-mt-en-zh' })).toBe(
      'local_nmt'
    );
    expect(resolveLlmExecutionPool({ provider: 'local_model', model: 'Auto-Match NMT (zh)' })).toBe(
      'local_nmt'
    );
  });

  it('keeps custom local GGUF models containing opus on local_llm and remote models on remote_api', () => {
    expect(
      resolveLlmExecutionPool({ provider: 'local', strategy: 'local', model: 'qwen-opus-7b.gguf' })
    ).toBe('local_llm');
    expect(resolveLlmExecutionPool({ provider: 'llama_cpp', model: 'opus-instruct-q4.gguf' })).toBe(
      'local_llm'
    );
    expect(resolveLlmExecutionPool({ provider: 'open_ai', model: 'gpt-4o-mini' })).toBe(
      'remote_api'
    );
  });
});

describe('LlmJobScheduler', () => {
  it('preempts normal-priority queued jobs with high-priority jobs within the same pool', async () => {
    const scheduler = new LlmJobScheduler();
    const executionOrder: string[] = [];
    const firstGate = Promise.withResolvers<void>();

    const first = scheduler.schedule(
      { config: { provider: 'local', model: 'qwen2.5-7b.gguf' }, priority: 'normal' },
      async () => {
        executionOrder.push('first-normal');
        await firstGate.promise;
      }
    );

    const queuedNormal = scheduler.schedule(
      { config: { provider: 'local', model: 'qwen2.5-7b.gguf' }, priority: 'normal' },
      async () => {
        executionOrder.push('queued-normal');
      }
    );

    const queuedHigh = scheduler.schedule(
      { config: { provider: 'local', model: 'qwen2.5-7b.gguf' }, priority: 'high' },
      async () => {
        executionOrder.push('queued-high');
      }
    );

    await Promise.resolve();
    expect(executionOrder).toEqual(['first-normal']);

    firstGate.resolve();
    await Promise.all([first, queuedNormal, queuedHigh]);

    expect(executionOrder).toEqual(['first-normal', 'queued-high', 'queued-normal']);
  });

  it('runs local_nmt and local_llm concurrently while limiting each local pool to 1 and remote_api to 3', async () => {
    const scheduler = new LlmJobScheduler();
    const localLlmGate = Promise.withResolvers<void>();
    const localNmtGate = Promise.withResolvers<void>();
    let nmtStartedWhileLlmActive = false;

    const llmJob1 = scheduler.schedule(
      { config: { provider: 'local', model: 'qwen.gguf' }, priority: 'normal' },
      async () => {
        await localLlmGate.promise;
      }
    );
    const nmtJob1 = scheduler.schedule(
      {
        config: { provider: 'local', strategy: 'local_nmt', model: 'opus-mt-en-zh' },
        priority: 'normal',
      },
      async () => {
        nmtStartedWhileLlmActive = true;
        await localNmtGate.promise;
      }
    );

    await Promise.resolve();
    expect(nmtStartedWhileLlmActive).toBe(true);

    localLlmGate.resolve();
    localNmtGate.resolve();
    await Promise.all([llmJob1, nmtJob1]);

    // Remote API allows 3 concurrent jobs and queues the 4th
    const remoteGates = Array.from({ length: 4 }, () => Promise.withResolvers<void>());
    const remoteStarted: number[] = [];
    const remoteJobs = remoteGates.map((gate, idx) =>
      scheduler.schedule(
        { config: { provider: 'open_ai', model: 'gpt-4o' }, priority: 'normal' },
        async () => {
          remoteStarted.push(idx);
          await gate.promise;
        }
      )
    );

    await Promise.resolve();
    expect(remoteStarted).toEqual([0, 1, 2]);

    for (const gate of remoteGates) {
      gate.resolve();
    }
    await Promise.all(remoteJobs);
    expect(remoteStarted).toEqual([0, 1, 2, 3]);
  });

  it('short-circuits queued cancelled jobs before execution and before onStart', async () => {
    const scheduler = new LlmJobScheduler();
    const blocker = Promise.withResolvers<void>();
    let cancelRequested = false;

    const active = scheduler.schedule(
      { config: { provider: 'local', model: 'qwen.gguf' }, priority: 'normal' },
      async () => {
        await blocker.promise;
      }
    );

    const onStart = vi.fn();
    const onCancelledWhileQueued = vi.fn();
    const fn = vi.fn();

    const cancelled = scheduler.schedule(
      {
        config: { provider: 'local', model: 'qwen.gguf' },
        priority: 'normal',
        cancelCheck: () => cancelRequested,
        onStart,
        onCancelledWhileQueued,
      },
      fn
    );

    cancelRequested = true;
    blocker.resolve();

    await active;
    await expect(cancelled).rejects.toBeInstanceOf(LlmJobCancelledError);
    expect(onCancelledWhileQueued).toHaveBeenCalledTimes(1);
    expect(onStart).not.toHaveBeenCalled();
    expect(fn).not.toHaveBeenCalled();
  });

  it('aborts waiting queued jobs immediately when AbortSignal triggers', async () => {
    const scheduler = new LlmJobScheduler();
    const blocker = Promise.withResolvers<void>();
    const abortController = new AbortController();

    const active = scheduler.schedule(
      { config: { provider: 'local', model: 'qwen.gguf' }, priority: 'normal' },
      async () => {
        await blocker.promise;
      }
    );

    const onCancelledWhileQueued = vi.fn();
    const fn = vi.fn();
    const aborted = scheduler.schedule(
      {
        config: { provider: 'local', model: 'qwen.gguf' },
        priority: 'normal',
        signal: abortController.signal,
        onCancelledWhileQueued,
      },
      fn
    );

    abortController.abort();
    await expect(aborted).rejects.toBeInstanceOf(LlmJobCancelledError);
    expect(onCancelledWhileQueued).toHaveBeenCalledTimes(1);
    expect(fn).not.toHaveBeenCalled();

    blocker.resolve();
    await active;
  });
});
