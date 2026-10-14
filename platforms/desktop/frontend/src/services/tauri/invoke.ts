import { invoke } from '@tauri-apps/api/core';
import { normalizeError } from '../../utils/errorUtils';
import type {
  KnownTauriCommandName,
  TauriCommandArgs,
  TauriCommandResult,
  TauriCommandsWithArgs,
  TauriCommandsWithoutArgs,
} from './contracts';
import { notifySyncLocalChangeForCommand } from './syncLocalChangeBus';

export interface NormalizedTauriError extends Error {
  code?: string;
  cause?: unknown;
}

export async function invokeTauri<TCommand extends TauriCommandsWithoutArgs>(
  command: TCommand
): Promise<TauriCommandResult<TCommand>>;
export async function invokeTauri<TCommand extends TauriCommandsWithArgs>(
  command: TCommand,
  args: TauriCommandArgs<TCommand>
): Promise<TauriCommandResult<TCommand>>;
export async function invokeTauri<TCommand extends KnownTauriCommandName>(
  command: TCommand,
  args?: TauriCommandArgs<TCommand>
): Promise<TauriCommandResult<TCommand>> {
  try {
    const result =
      args === undefined
        ? invoke<TauriCommandResult<TCommand>>(command)
        : invoke<TauriCommandResult<TCommand>>(command, args as Record<string, unknown>);
    const resolved = await result;
    notifySyncLocalChangeForCommand(command);
    return resolved;
  } catch (error) {
    const normalized = normalizeError(error);
    const err: NormalizedTauriError = new Error(normalized.message);
    if (normalized.code) {
      err.code = normalized.code;
    }
    err.cause = error;
    throw err;
  }
}
