/**
 * A helper to safely serialize arguments, particularly JavaScript Error objects
 * which become {} when simply passed to JSON.stringify.
 */
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
  } catch (err) {
    return ' [Unserializable Object(s) - possibly circular references]';
  }
}

type LogLevel = 'debug' | 'info' | 'warn' | 'error';
type PluginLogModule = typeof import('@tauri-apps/plugin-log');

let pluginLogModulePromise: Promise<PluginLogModule | null> | null = null;

async function getPluginLogModule(): Promise<PluginLogModule | null> {
  if (!pluginLogModulePromise) {
    pluginLogModulePromise = import('@tauri-apps/plugin-log')
      .then((module) => module)
      .catch(() => null);
  }

  return pluginLogModulePromise;
}

async function writeLog(level: LogLevel, message: string, ...args: unknown[]) {
  const formatted = message + serializeArgs(args);
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
  debug: async (message: string, ...args: unknown[]) => {
    browserConsole.debug(message, ...args);
    await writeLog('debug', message, ...args);
  },
  info: async (message: string, ...args: unknown[]) => {
    browserConsole.info(message, ...args);
    await writeLog('info', message, ...args);
  },
  warn: async (message: string, ...args: unknown[]) => {
    browserConsole.warn(message, ...args);
    await writeLog('warn', message, ...args);
  },
  error: async (message: string, ...args: unknown[]) => {
    browserConsole.error(message, ...args);
    await writeLog('error', message, ...args);
  }
};
