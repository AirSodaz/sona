import { getPluginLogModule } from '../services/tauri/platform/log';
import type { AppLogLevel } from '../types/config';
import { normalizeLogLevel, shouldWriteLogLevel } from './logLevel';

const browserConsole = globalThis.console;
const sensitiveKey =
  /^(api[-_]?key|(?:access|refresh)[-_]?token|token|authorization|password|secret|webhook[-_]?secret|config|headers|text|transcript|audio|payload)$/i;
let currentLogLevel: AppLogLevel = 'info';

function sanitizeText(value: string): string {
  return value
    .replace(/\b(?:https?|wss?):\/\/[^\s"'<>]+/gi, '[redacted-url]')
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]+/gi, 'Bearer [redacted]')
    .replace(
      /(api[_-]?key|access[_-]?token|token|authorization|password|secret)(\s*[=:]\s*)([^&\s,;"'<>]+)/gi,
      '$1$2[redacted]'
    )
    .slice(0, 4096);
}

/** Bound diagnostics and retain nested Error details without invoking toJSON. */
function normalizeValue(value: unknown, seen = new WeakSet<object>(), depth = 0): unknown {
  if (typeof value === 'string') return sanitizeText(value);
  if (typeof value === 'bigint') return value.toString();
  if (value === null || typeof value === 'number' || typeof value === 'boolean') return value;
  if (typeof value !== 'object') return String(value);
  if (depth >= 6) return '[max-depth]';
  if (seen.has(value)) return '[circular]';
  seen.add(value);
  try {
    if (Array.isArray(value)) {
      return value.slice(0, 50).map((entry) => normalizeValue(entry, seen, depth + 1));
    }
    const keys =
      value instanceof Error
        ? [...new Set(['name', 'message', 'stack', 'cause', ...Object.getOwnPropertyNames(value)])]
        : Object.keys(value);
    return Object.fromEntries(
      keys
        .slice(0, 50)
        .map((key) => [
          key,
          sensitiveKey.test(key)
            ? '[redacted]'
            : normalizeValue((value as Record<string, unknown>)[key], seen, depth + 1),
        ])
    );
  } catch {
    return '[unserializable]';
  } finally {
    seen.delete(value);
  }
}

export function setLoggerLevel(level: unknown): AppLogLevel {
  currentLogLevel = normalizeLogLevel(level);
  return currentLogLevel;
}

export function getLoggerLevel(): AppLogLevel {
  return currentLogLevel;
}

async function writeLog(level: AppLogLevel, message: unknown, ...args: unknown[]): Promise<void> {
  if (!shouldWriteLogLevel(level, currentLogLevel)) return;
  // Logging must never reject, including serializer, console and plugin failures.
  try {
    const text = typeof message === 'string' ? sanitizeText(message) : 'diagnostic';
    const match = /^\[([^\]]+)\]\s*(.*)$/.exec(text);
    const detail = match?.[2] ?? text;
    const context = typeof message === 'string' ? args : [message, ...args];
    const entry = {
      schema_version: 1,
      timestamp_ms: Date.now(),
      source: 'frontend',
      component: match?.[1] ?? 'Client',
      event: /^[a-z][a-z0-9]*(?:_[a-z0-9]+)+$/.test(detail) ? detail : 'message',
      message: text,
      context: normalizeValue(context.length === 1 ? context[0] : { args: context }),
    };
    const formatted = JSON.stringify(entry);
    try {
      browserConsole[level === 'trace' ? 'debug' : level](formatted);
    } catch {
      // A broken devtools sink must not prevent the file sink.
    }
    const plugin = await getPluginLogModule();
    if (plugin) await plugin[level](formatted);
  } catch {
    // Do not recursively log a failed log write.
  }
}

export const logger = {
  trace: (message: unknown, ...args: unknown[]) => writeLog('trace', message, ...args),
  debug: (message: unknown, ...args: unknown[]) => writeLog('debug', message, ...args),
  info: (message: unknown, ...args: unknown[]) => writeLog('info', message, ...args),
  warn: (message: unknown, ...args: unknown[]) => writeLog('warn', message, ...args),
  error: (message: unknown, ...args: unknown[]) => writeLog('error', message, ...args),
};
