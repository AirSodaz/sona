import { invoke } from '@tauri-apps/api/core';
import type { TauriCommandName } from './commands';

export async function invokeTauri<TResult>(
  command: TauriCommandName,
  args?: Record<string, unknown>,
): Promise<TResult> {
  return args === undefined
    ? invoke<TResult>(command)
    : invoke<TResult>(command, args);
}
