export interface NormalizedError {
  code?: string;
  message: string;
}

export interface AppErrorInput {
  code: string;
  messageKey: string;
  messageParams?: Record<string, unknown>;
  cause?: unknown;
  showCause?: boolean;
  titleKey?: string;
  primaryActionLabelKey?: string;
  cancelLabelKey?: string;
}

export interface ErrorDialogViewModel {
  title: string;
  message: string;
  details?: string;
  primaryLabel: string;
  cancelLabel?: string;
  hasPrimaryAction: boolean;
}

interface BuiltErrorDialogOptions {
  details?: string;
  message: string;
  title: string;
}

type TranslateFn = (key: string, options?: Record<string, unknown>) => string;

const UNKNOWN_ERROR_MESSAGE = 'Unknown error';
const MESSAGE_KEYS = ['message', 'error', 'reason', 'details', 'detail', 'cause'] as const;
const CODE_KEYS = ['code', 'errorCode', 'status', 'statusCode'] as const;

export function normalizeError(error: unknown): NormalizedError {
  const message = extractMessageCandidate(error) ?? UNKNOWN_ERROR_MESSAGE;
  const code = extractCodeCandidate(error);

  return {
    code,
    message: normalizeTauriErrorMessage(message),
  };
}

export function extractErrorMessage(error: unknown): string {
  return normalizeError(error).message;
}

export function getErrorDetails(error: unknown): string | undefined {
  const normalized = normalizeError(error);

  if (normalized.message === UNKNOWN_ERROR_MESSAGE) {
    return normalized.code ? `[${normalized.code}]` : undefined;
  }

  if (normalized.code && !normalized.message.includes(normalized.code)) {
    return `${normalized.message} [${normalized.code}]`;
  }

  return normalized.message;
}

export function buildErrorDialogOptions(t: TranslateFn, input: AppErrorInput): BuiltErrorDialogOptions {
  const fallbackMessage = t('errors.common.operation_failed', {
    defaultValue: 'The operation could not be completed.',
  });
  const title = t(input.titleKey ?? 'common.error', { defaultValue: 'Error' });
  const message = t(input.messageKey, {
    ...(input.messageParams ?? {}),
    defaultValue: fallbackMessage,
  });
  const details = input.showCause === false ? undefined : getErrorDetails(input.cause);

  return {
    title,
    message,
    details: details && details !== message ? details : undefined,
  };
}

export function buildErrorDialogViewModel(t: TranslateFn, input: AppErrorInput): ErrorDialogViewModel {
  const { title, message, details } = buildErrorDialogOptions(t, input);
  const hasPrimaryAction = Boolean(input.primaryActionLabelKey);

  return {
    title,
    message,
    details,
    primaryLabel: hasPrimaryAction
      ? t(input.primaryActionLabelKey!, { defaultValue: 'Continue' })
      : t('common.ok', { defaultValue: 'OK' }),
    cancelLabel: hasPrimaryAction
      ? t(input.cancelLabelKey ?? 'common.cancel', { defaultValue: 'Cancel' })
      : undefined,
    hasPrimaryAction,
  };
}

function extractMessageCandidate(error: unknown, visited = new WeakSet<object>()): string | undefined {
  if (typeof error === 'string') {
    return error.trim() || undefined;
  }

  if (typeof error === 'number' || typeof error === 'boolean' || typeof error === 'bigint') {
    return String(error);
  }

  if (error instanceof Error) {
    if (error.message?.trim()) {
      return error.message.trim();
    }

    const errorWithCause = error as Error & { cause?: unknown };
    if (errorWithCause.cause !== undefined) {
      return extractMessageCandidate(errorWithCause.cause, visited);
    }
  }

  if (Array.isArray(error)) {
    for (const item of error) {
      const candidate = extractMessageCandidate(item, visited);
      if (candidate) {
        return candidate;
      }
    }
  }

  if (typeof error === 'object' && error !== null) {
    if (visited.has(error)) {
      return undefined;
    }

    visited.add(error);
    const record = error as Record<string, unknown>;

    for (const key of MESSAGE_KEYS) {
      const candidate = extractMessageCandidate(record[key], visited);
      if (candidate) {
        return candidate;
      }
    }

    try {
      const serialized = JSON.stringify(error);
      if (serialized && serialized !== '{}' && serialized !== '[]') {
        return serialized;
      }
    } catch {
      // Ignore serialization failures and fall through to the default message.
    }
  }

  return undefined;
}

function extractCodeCandidate(error: unknown): string | undefined {
  if (typeof error !== 'object' || error === null) {
    return undefined;
  }

  const record = error as Record<string, unknown>;

  for (const key of CODE_KEYS) {
    const value = record[key];
    if (typeof value === 'string' && value.trim()) {
      return value.trim();
    }
    if (typeof value === 'number') {
      return String(value);
    }
  }

  return undefined;
}

function normalizeTauriErrorMessage(message: string): string {
  const trimmed = message.trim();
  const tauriPrefix = 'error invoking command';
  const marker = 'Caused by:';

  if (trimmed.toLowerCase().startsWith(tauriPrefix) && trimmed.includes(marker)) {
    return trimmed.slice(trimmed.lastIndexOf(marker) + marker.length).trim();
  }

  return trimmed;
}
