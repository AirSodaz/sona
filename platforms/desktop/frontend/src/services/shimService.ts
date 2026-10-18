import type { PlatformContext } from '../platform';
import { getPlatform, TauriCommand, UnsupportedCapabilityError } from '../platform';
import type { DesktopShimStatus, ShimChannel } from '../types/shim';

export class ShimService {
  constructor(private readonly platform?: PlatformContext) {}

  isSupported(): boolean {
    return (
      typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
    );
  }

  async getStatus(): Promise<DesktopShimStatus | null> {
    if (!this.isSupported()) {
      return null;
    }
    try {
      const platform = this.platform ?? getPlatform();
      return await platform.transport.invoke<DesktopShimStatus>(TauriCommand.system.getShimStatus);
    } catch (err) {
      const isUnsupported =
        err instanceof UnsupportedCapabilityError ||
        (err instanceof Error && err.name === 'UnsupportedCapabilityError') ||
        (typeof err === 'object' &&
          err !== null &&
          'code' in err &&
          err.code === 'UNSUPPORTED_CAPABILITY');
      if (isUnsupported) {
        return null;
      }
      throw err;
    }
  }

  async install(): Promise<void> {
    const platform = this.platform ?? getPlatform();
    await platform.transport.invoke<void>(TauriCommand.system.installShim);
  }

  async uninstall(): Promise<void> {
    const platform = this.platform ?? getPlatform();
    await platform.transport.invoke<void>(TauriCommand.system.uninstallShim);
  }

  async setDefaultChannel(channel: ShimChannel): Promise<void> {
    const platform = this.platform ?? getPlatform();
    await platform.transport.invoke<void>(TauriCommand.system.setDefaultChannel, {
      channel,
    });
  }
}

export function createShimService(platform?: PlatformContext): ShimService {
  return new ShimService(platform);
}

export const shimService = createShimService();
