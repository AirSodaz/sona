import type { PlatformContext } from '../../types';
import { createBrowserPlatformPorts } from './BrowserPlatformPorts';
import { WebSocketApiTransport, type WebSocketApiTransportOptions } from './WebSocketApiTransport';

export * from './BrowserPlatformPorts';
export * from './errors';
export * from './HttpApiTransport';
export * from './WebSocketApiTransport';

export function createWebPlatform(
  options: WebSocketApiTransportOptions = {}
): PlatformContext & { transport: WebSocketApiTransport } {
  const transport = new WebSocketApiTransport(options);
  const ports = createBrowserPlatformPorts();
  return {
    transport,
    ports,
  };
}
