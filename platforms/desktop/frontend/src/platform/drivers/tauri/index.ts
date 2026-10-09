import type { PlatformContext } from '../../types';
import { createTauriPlatformPorts } from './TauriPlatformPorts';
import { TauriTransport } from './TauriTransport';

export * from './TauriPlatformPorts';
export * from './TauriTransport';

export function createTauriPlatform(): PlatformContext {
  return {
    transport: new TauriTransport(),
    ports: createTauriPlatformPorts(),
  };
}
