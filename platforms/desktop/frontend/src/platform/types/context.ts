import type { IPlatformPorts } from './ports';
import type { ITransport } from './transport';

export interface PlatformContext {
  transport: ITransport;
  ports: IPlatformPorts;
}
