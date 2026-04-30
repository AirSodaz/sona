import { invoke } from '@tauri-apps/api/core';
import type {
  KnownTauriCommandName,
  TauriCommandArgs,
  TauriCommandResult,
  TauriCommandsWithArgs,
  TauriCommandsWithoutArgs,
} from './contracts';

export async function invokeTauri<TCommand extends TauriCommandsWithoutArgs>(
  command: TCommand,
): Promise<TauriCommandResult<TCommand>>;
export async function invokeTauri<TCommand extends TauriCommandsWithArgs>(
  command: TCommand,
  args: TauriCommandArgs<TCommand>,
): Promise<TauriCommandResult<TCommand>>;
export async function invokeTauri<TCommand extends KnownTauriCommandName>(
  command: TCommand,
  args?: TauriCommandArgs<TCommand>,
): Promise<TauriCommandResult<TCommand>> {
  return args === undefined
    ? invoke<TauriCommandResult<TCommand>>(command)
    : invoke<TauriCommandResult<TCommand>>(command, args as Record<string, unknown>);
}
