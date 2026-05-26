import type { TaskLedgerRecord } from '../types/taskLedger';
import { normalizeError } from '../utils/errorUtils';
import { patchTaskLedgerRecord } from './taskLedgerRuntime';

/** Records retry preflight failures on the original ledger task, then rethrows a normalized error. */
export function handleTaskRetryPreflightFailure(task: TaskLedgerRecord, error: unknown): never {
  const normalizedError = normalizeError(error);

  patchTaskLedgerRecord(task.id, {
    status: 'failed',
    progress: 0,
    cancelable: false,
    retryable: true,
    errorMessage: normalizedError.message,
  });

  throw Object.assign(new Error(normalizedError.message), { cause: error });
}
