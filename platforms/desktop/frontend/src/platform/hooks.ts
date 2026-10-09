import { useMemo } from 'react';
import { getPlatform, type PlatformContext } from './context';
import type { ITransport } from './types/transport';

/**
 * React hook that returns the active PlatformContext.
 */
export function usePlatform(): PlatformContext {
  return useMemo(() => getPlatform(), []);
}

/**
 * React hook that returns the active ITransport instance for RPC/IPC operations.
 */
export function useTransport(): ITransport {
  const platform = usePlatform();
  return platform.transport;
}
