/**
 * A helper to safely serialize arguments, particularly JavaScript Error objects
 * which become {} when simply passed to JSON.stringify.
 */
import type { AppLogLevel } from '../types/config';
import { normalizeLogLevel, shouldWriteLogLevel } from './logLevel';

const browserConsole = globalThis.console;

function serializeArgs(args: unknown[]): string {
  if (!args || args.length === 0) return '';

  const serialized = args.map((arg) => {
    if (arg instanceof Error) {
      return {
        name: arg.name,
        message: arg.message,
        stack: arg.stack,
      };
    }
    return arg;
  });

  try {
    return ` ${JSON.stringify(serialized)}`;
  } catch {
    return ' [Unserializable Object(s) - possibly circular references]';
  }
}

type LogLevel = AppLogLevel;
type PluginLogModule = typeof import('@tauri-apps/plugin-log');

let pluginLogModulePromise: Promise<PluginLogModule | null> | null = null;
let currentLogLevel: AppLogLevel = 'info';

export function setLoggerLevel(level: unknown): AppLogLevel {
  currentLogLevel = normalizeLogLevel(level);
  return currentLogLevel;
}

export function getLoggerLevel(): AppLogLevel {
  return currentLogLevel;
}

async function getPluginLogModule(): Promise<PluginLogModule | null> {
  if (!pluginLogModulePromise) {
    pluginLogModulePromise = import('@tauri-apps/plugin-log')
      .then((module) => module)
      .catch(() => null);
  }

  return pluginLogModulePromise;
}

async function writeLog(level: LogLevel, message: unknown, ...args: unknown[]) {
  if (!shouldWriteLogLevel(level, currentLogLevel)) {
    return;
  }

  let formatted: string;
  if (message instanceof Error) {
    formatted = `${message.name}: ${message.message}\nStack: ${message.stack ?? ''}`;
  } else if (typeof message === 'string') {
    formatted = message;
  } else {
    try {
      formatted = JSON.stringify(message);
    } catch {
      formatted = String(message);
    }
  }

  if (args.length > 0) {
    formatted += serializeArgs(args);
  }

  const plugin = await getPluginLogModule();

  if (!plugin) {
    return;
  }

  const logMethod = plugin[level];
  await logMethod(formatted).catch(() => {});
}

/**
 * A dedicated logging utility that uses tauri-plugin-log.
 * This logs directly to the standard system application data directory
 * in a structured format as configured by the Rust backend.
 */
export const logger = {
  trace: async (message: unknown, ...args: unknown[]) => {
    if (!shouldWriteLogLevel('trace', currentLogLevel)) return;
    browserConsole.debug(message, ...args);
    await writeLog('trace', message, ...args);
  },
  debug: async (message: unknown, ...args: unknown[]) => {
    if (!shouldWriteLogLevel('debug', currentLogLevel)) return;
    browserConsole.debug(message, ...args);
    await writeLog('debug', message, ...args);
  },
  info: async (message: unknown, ...args: unknown[]) => {
    if (!shouldWriteLogLevel('info', currentLogLevel)) return;
    browserConsole.info(message, ...args);
    await writeLog('info', message, ...args);
  },
  warn: async (message: unknown, ...args: unknown[]) => {
    if (!shouldWriteLogLevel('warn', currentLogLevel)) return;
    browserConsole.warn(message, ...args);
    await writeLog('warn', message, ...args);
  },
  error: async (message: unknown, ...args: unknown[]) => {
    if (!shouldWriteLogLevel('error', currentLogLevel)) return;
    browserConsole.error(message, ...args);
    await writeLog('error', message, ...args);
  }
};
