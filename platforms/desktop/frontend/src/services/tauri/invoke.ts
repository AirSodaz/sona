import { invoke } from '@tauri-apps/api/core';
import type {
  KnownTauriCommandName,
  TauriCommandArgs,
  TauriCommandResult,
  TauriCommandsWithArgs,
  TauriCommandsWithoutArgs,
} from './contracts';
import { notifySyncLocalChangeForCommand } from '../syncLocalChangeBus';

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
  const result = args === undefined
    ? invoke<TauriCommandResult<TCommand>>(command)
    : invoke<TauriCommandResult<TCommand>>(command, args as Record<string, unknown>);
  const resolved = await result;
  notifySyncLocalChangeForCommand(command);
  return resolved;
}
