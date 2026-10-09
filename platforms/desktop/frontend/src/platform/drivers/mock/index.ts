import type { PlatformContext } from '../../context';
import type { IPlatformPorts } from '../../types';
import { createMockPlatformPorts } from './MockPlatformPorts';
import { MockTransport } from './MockTransport';

export * from './MockPlatformPorts';
export * from './MockTransport';

export function createMockPlatform(
  overrides?: Partial<PlatformContext>
): PlatformContext & { transport: MockTransport; ports: IPlatformPorts } {
  const transport = (overrides?.transport as MockTransport) ?? new MockTransport();
  const ports = overrides?.ports ?? createMockPlatformPorts();
  return {
    transport,
    ports,
  };
}
