import type { ITransport, UnlistenFn } from '../../types/transport';
import { HttpApiTransport, type HttpApiTransportOptions } from './HttpApiTransport';

export interface WebSocketApiTransportOptions extends HttpApiTransportOptions {
  wsUrl?: string;
  autoConnect?: boolean;
}

export class WebSocketApiTransport implements ITransport {
  private readonly httpTransport: HttpApiTransport;
  private readonly wsUrl: string;
  private socket: WebSocket | null = null;
  private readonly listeners = new Map<string, Set<(payload: unknown) => void>>();

  constructor(options: WebSocketApiTransportOptions = {}) {
    this.httpTransport = new HttpApiTransport(options);
    const base = options.baseUrl ?? 'http://127.0.0.1:14200';
    this.wsUrl = options.wsUrl ?? base.replace(/^http/, 'ws') + '/v1/events';

    if (options.autoConnect && typeof WebSocket !== 'undefined') {
      this.connect();
    }
  }

  connect(): void {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      return;
    }

    try {
      this.socket = new WebSocket(this.wsUrl);
      this.socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data) as { event?: string; payload?: unknown };
          if (data && typeof data.event === 'string') {
            const handlers = this.listeners.get(data.event);
            if (handlers) {
              for (const handler of Array.from(handlers)) {
                handler(data.payload);
              }
            }
          }
        } catch {
          // ignore non-json messages
        }
      };
    } catch {
      // WebSocket connection failed (e.g. server not running or web worker)
    }
  }

  async invoke<TResult = unknown, TArgs = unknown>(
    command: string,
    args?: TArgs
  ): Promise<TResult> {
    return this.httpTransport.invoke<TResult, TArgs>(command, args);
  }

  listen<TPayload = unknown>(event: string, handler: (payload: TPayload) => void): UnlistenFn {
    let handlers = this.listeners.get(event);
    if (!handlers) {
      handlers = new Set();
      this.listeners.set(event, handlers);
    }
    const typedHandler = handler as (payload: unknown) => void;
    handlers.add(typedHandler);

    // Also register on httpTransport for in-process emit compatibility
    const httpUnlisten = this.httpTransport.listen(event, handler);

    return () => {
      handlers.delete(typedHandler);
      if (handlers.size === 0) {
        this.listeners.delete(event);
      }
      httpUnlisten();
    };
  }

  async emit<TPayload = unknown>(event: string, payload?: TPayload): Promise<void> {
    await this.httpTransport.emit(event, payload);

    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify({ event, payload }));
    }
  }

  disconnect(): void {
    if (this.socket) {
      this.socket.close();
      this.socket = null;
    }
  }
}
