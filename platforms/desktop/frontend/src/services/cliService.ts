import type { CliStatus } from '../types/cli';
import { TauriCommand } from './tauri/commands';
import { invokeTauri } from './tauri/invoke';

export type { CliStatus };

export const cliService = {
  async getStatus(): Promise<CliStatus> {
    return invokeTauri(TauriCommand.cli.getStatus);
  },

  async setPathEnabled(enabled: boolean): Promise<CliStatus> {
    return invokeTauri(TauriCommand.cli.setPathEnabled, { enabled });
  },

  async getCompletionScript(shell: string): Promise<string> {
    return invokeTauri(TauriCommand.cli.getCompletionScript, { shell });
  },
};
