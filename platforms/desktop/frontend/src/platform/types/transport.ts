export type UnlistenFn = () => void;

export interface ITransport {
  /**
   * Invokes an RPC/IPC command with optional arguments and returns the result.
   */
  invoke<TResult = unknown, TArgs = unknown>(command: string, args?: TArgs): Promise<TResult>;

  /**
   * Subscribes to an event emitted by the platform or backend.
   * Returns a synchronous unlisten function that unregisters the subscription.
   */
  listen<TPayload = unknown>(event: string, handler: (payload: TPayload) => void): UnlistenFn;

  /**
   * Emits an event to the backend or other platform listeners.
   */
  emit<TPayload = unknown>(event: string, payload?: TPayload): Promise<void>;
}
