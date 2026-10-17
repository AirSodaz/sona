import { getPlatform, TauriCommand } from '../platform';
import type { DesktopShimStatus } from '../types/shim';

export const shimService = {
  async getStatus(): Promise<DesktopShimStatus> {
    return getPlatform().transport.invoke<DesktopShimStatus>(TauriCommand.system.getShimStatus);
  },

  async install(): Promise<void> {
    await getPlatform().transport.invoke<void>(TauriCommand.system.installShim);
  },

  async uninstall(): Promise<void> {
    await getPlatform().transport.invoke<void>(TauriCommand.system.uninstallShim);
  },

  async setDefaultChannel(channel: 'stable' | 'nightly'): Promise<void> {
    await getPlatform().transport.invoke<void>(TauriCommand.system.setDefaultChannel, { channel });
  },
};
