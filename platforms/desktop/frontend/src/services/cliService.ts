import type { CliStatus, ShellCompletionStatus, ShellKind } from '../types/cli';
import { TauriCommand } from './tauri/commands';
import { invokeTauri } from './tauri/invoke';

export type { CliStatus, ShellCompletionStatus, ShellKind };

export const cliService = {
  async getStatus(): Promise<CliStatus> {
    return invokeTauri(TauriCommand.cli.getStatus);
  },

  async setPathEnabled(enabled: boolean): Promise<CliStatus> {
    return invokeTauri(TauriCommand.cli.setPathEnabled, { enabled });
  },

  async switchPathToCurrent(): Promise<CliStatus> {
    return invokeTauri(TauriCommand.cli.switchPathToCurrent);
  },

  async removePathEntry(pathDir: string): Promise<CliStatus> {
    return invokeTauri(TauriCommand.cli.removePathEntry, { pathDir });
  },

  async getCompletionScript(shell: string): Promise<string> {
    return invokeTauri(TauriCommand.cli.getCompletionScript, { shell });
  },

  async getCompletionStatuses(): Promise<ShellCompletionStatus[]> {
    return invokeTauri(TauriCommand.cli.getCompletionStatuses);
  },

  async installCompletion(shell: ShellKind): Promise<ShellCompletionStatus> {
    return invokeTauri(TauriCommand.cli.installCompletion, { shell });
  },

  async uninstallCompletion(shell: ShellKind): Promise<ShellCompletionStatus> {
    return invokeTauri(TauriCommand.cli.uninstallCompletion, { shell });
  },
};
