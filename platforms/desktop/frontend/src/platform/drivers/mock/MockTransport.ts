import type { ITransport, UnlistenFn } from '../../types/transport';

export interface CommandInvocation<TArgs = unknown> {
  command: string;
  args?: TArgs;
  timestamp: number;
}

export interface EmittedEvent<TPayload = unknown> {
  event: string;
  payload?: TPayload;
  timestamp: number;
}

export type CommandHandler<TResult = unknown, TArgs = unknown> = (
  args?: TArgs
) => Promise<TResult> | TResult;

export class MockTransport implements ITransport {
  private commandHandlers = new Map<string, CommandHandler<unknown, unknown>>();
  private defaultHandler: ((command: string, args?: unknown) => Promise<unknown> | unknown) | null =
    null;
  private listeners = new Map<string, Set<(payload: unknown) => void>>();
  private invocations: CommandInvocation[] = [];
  private emissions: EmittedEvent[] = [];

  async invoke<TResult = unknown, TArgs = unknown>(
    command: string,
    args?: TArgs
  ): Promise<TResult> {
    this.invocations.push({
      command,
      args,
      timestamp: Date.now(),
    });

    const handler = this.commandHandlers.get(command);
    if (handler) {
      return (await handler(args)) as TResult;
    }

    if (this.defaultHandler) {
      return (await this.defaultHandler(command, args)) as TResult;
    }

    throw new Error(`[MockTransport] Unhandled command: "${command}"`);
  }

  listen<TPayload = unknown>(event: string, handler: (payload: TPayload) => void): UnlistenFn {
    let set = this.listeners.get(event);
    if (!set) {
      set = new Set();
      this.listeners.set(event, set);
    }

    const typedHandler = handler as (payload: unknown) => void;
    set.add(typedHandler);

    return () => {
      set.delete(typedHandler);
      if (set.size === 0) {
        this.listeners.delete(event);
      }
    };
  }

  async emit<TPayload = unknown>(event: string, payload?: TPayload): Promise<void> {
    this.emissions.push({
      event,
      payload,
      timestamp: Date.now(),
    });
  }

  // --- Testing Probes & Helpers ---

  setCommandHandler<TResult = unknown, TArgs = unknown>(
    command: string,
    handler: CommandHandler<TResult, TArgs>
  ): void {
    this.commandHandlers.set(command, handler as CommandHandler<unknown, unknown>);
  }

  setDefaultCommandHandler(
    handler: (command: string, args?: unknown) => Promise<unknown> | unknown
  ): void {
    this.defaultHandler = handler;
  }

  removeCommandHandler(command: string): void {
    this.commandHandlers.delete(command);
  }

  mockEmit<TPayload = unknown>(event: string, payload: TPayload): void {
    const set = this.listeners.get(event);
    if (!set) {
      return;
    }
    for (const handler of Array.from(set)) {
      handler(payload);
    }
  }

  getInvocations(): readonly CommandInvocation[] {
    return [...this.invocations];
  }

  getInvocationsFor(command: string): CommandInvocation[] {
    return this.invocations.filter((inv) => inv.command === command);
  }

  getEmissions(): readonly EmittedEvent[] {
    return [...this.emissions];
  }

  hasListener(event: string): boolean {
    const set = this.listeners.get(event);
    return Boolean(set && set.size > 0);
  }

  clearInvocations(): void {
    this.invocations = [];
  }

  clearEmissions(): void {
    this.emissions = [];
  }

  clearHandlers(): void {
    this.commandHandlers.clear();
    this.defaultHandler = null;
  }

  clearListeners(): void {
    this.listeners.clear();
  }

  clearAll(): void {
    this.clearInvocations();
    this.clearEmissions();
    this.clearHandlers();
    this.clearListeners();
  }
}
