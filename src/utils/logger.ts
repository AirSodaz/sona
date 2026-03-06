import { debug as tauriDebug, info as tauriInfo, warn as tauriWarn, error as tauriError } from '@tauri-apps/plugin-log';

/**
 * A helper to safely serialize arguments, particularly JavaScript Error objects
 * which become {} when simply passed to JSON.stringify.
 */
function serializeArgs(args: any[]): string {
  if (!args || args.length === 0) return '';
  const serialized = args.map(arg => {
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

/**
 * A dedicated logging utility that uses tauri-plugin-log.
 * This logs directly to the standard system application data directory
 * in a structured format as configured by the Rust backend.
 */
export const logger = {
  debug: async (message: string, ...args: any[]) => {
    const formatted = message + serializeArgs(args);
    await tauriDebug(formatted).catch(() => {});
  },
  info: async (message: string, ...args: any[]) => {
    const formatted = message + serializeArgs(args);
    await tauriInfo(formatted).catch(() => {});
  },
  warn: async (message: string, ...args: any[]) => {
    const formatted = message + serializeArgs(args);
    await tauriWarn(formatted).catch(() => {});
  },
  error: async (message: string, ...args: any[]) => {
    const formatted = message + serializeArgs(args);
    await tauriError(formatted).catch(() => {});
  }
};
