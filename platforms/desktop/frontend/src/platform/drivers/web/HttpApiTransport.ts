import type { ITransport, UnlistenFn } from '../../types/transport';
import { UnsupportedCapabilityError } from './errors';

export interface HttpApiTransportOptions {
  baseUrl?: string;
  apiKey?: string;
  fetchFn?: typeof fetch;
}

export class HttpApiTransport implements ITransport {
  private readonly baseUrl: string;
  private readonly apiKey: string;
  private readonly fetchFn: typeof fetch;
  private readonly listeners = new Map<string, Set<(payload: unknown) => void>>();

  constructor(options: HttpApiTransportOptions = {}) {
    this.baseUrl = (options.baseUrl ?? 'http://127.0.0.1:14200').replace(/\/+$/, '');
    this.apiKey = options.apiKey ?? '';
    this.fetchFn =
      options.fetchFn ??
      (typeof fetch !== 'undefined'
        ? fetch.bind(globalThis)
        : async () => {
            throw new Error('[HttpApiTransport] No fetch implementation available.');
          });
  }

  async invoke<TResult = unknown, TArgs = unknown>(
    command: string,
    args?: TArgs
  ): Promise<TResult> {
    const route = this.resolveRoute(command);
    if (!route) {
      throw new UnsupportedCapabilityError(
        command,
        `[HttpApiTransport] Command "${command}" is not implemented by the API server and is desktop-only.`
      );
    }

    const url = `${this.baseUrl}${route.path}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };
    if (this.apiKey) {
      headers.Authorization = `Bearer ${this.apiKey}`;
    }

    const init: RequestInit = {
      method: route.method,
      headers,
    };

    if (route.method === 'POST') {
      init.body = JSON.stringify(args ?? {});
    }

    const response = await this.fetchFn(url, init);
    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(
        `[HttpApiTransport] Request to ${url} failed with status ${response.status}: ${errorText}`
      );
    }

    return (await response.json()) as TResult;
  }

  listen<TPayload = unknown>(event: string, handler: (payload: TPayload) => void): UnlistenFn {
    let handlers = this.listeners.get(event);
    if (!handlers) {
      handlers = new Set();
      this.listeners.set(event, handlers);
    }
    const typedHandler = handler as (payload: unknown) => void;
    handlers.add(typedHandler);

    return () => {
      handlers.delete(typedHandler);
      if (handlers.size === 0) {
        this.listeners.delete(event);
      }
    };
  }

  async emit<TPayload = unknown>(event: string, payload?: TPayload): Promise<void> {
    const handlers = this.listeners.get(event);
    if (!handlers) return;
    for (const handler of Array.from(handlers)) {
      handler(payload);
    }
  }

  private resolveRoute(command: string): { method: 'GET' | 'POST'; path: string } | null {
    switch (command) {
      case 'v1/health':
      case 'health':
        return { method: 'GET', path: '/health' };
      case 'v1/transcriptions':
      case 'transcribe':
        return { method: 'POST', path: '/v1/transcriptions' };
      case 'v1/transcriptions/jobs':
      case 'list_jobs':
        return { method: 'GET', path: '/v1/transcriptions/jobs' };
      case 'v1/llm/polish':
      case 'llm_polish':
        return { method: 'POST', path: '/v1/llm/polish' };
      case 'v1/llm/translate':
      case 'llm_translate':
        return { method: 'POST', path: '/v1/llm/translate' };
      default:
        return null;
    }
  }
}
