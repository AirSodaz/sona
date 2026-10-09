import type { KnownTauriCommandName, TauriCommandArgs } from '../../../services/tauri/contracts';
import { invokeTauri } from '../../../services/tauri/invoke';
import { emit as tauriEmit, listen as tauriListen } from '../../../services/tauri/platform/events';
import type { ITransport, UnlistenFn } from '../../types/transport';

export class TauriTransport implements ITransport {
  async invoke<TResult = unknown, TArgs = unknown>(
    command: string,
    args?: TArgs
  ): Promise<TResult> {
    const cmd = command as KnownTauriCommandName;
    // Cast through unknown to preserve strong typing at runtime boundaries without bare any
    const result =
      args === undefined
        ? await (invokeTauri as (c: KnownTauriCommandName) => Promise<unknown>)(cmd)
        : await (invokeTauri as (c: KnownTauriCommandName, a: unknown) => Promise<unknown>)(
            cmd,
            args as unknown as TauriCommandArgs<typeof cmd>
          );
    return result as unknown as TResult;
  }

  listen<TPayload = unknown>(event: string, handler: (payload: TPayload) => void): UnlistenFn {
    let unlisted = false;
    let tauriUnlisten: (() => void) | null = null;

    tauriListen(event, (eventObj) => {
      if (!unlisted) {
        handler(eventObj.payload as TPayload);
      }
    })
      .then((unlisten) => {
        if (unlisted) {
          unlisten();
        } else {
          tauriUnlisten = unlisten;
        }
      })
      .catch((error: unknown) => {
        console.error(`[TauriTransport] Failed to subscribe to event "${event}":`, error);
      });

    return () => {
      unlisted = true;
      if (tauriUnlisten) {
        tauriUnlisten();
        tauriUnlisten = null;
      }
    };
  }

  async emit<TPayload = unknown>(event: string, payload?: TPayload): Promise<void> {
    await tauriEmit(event, payload);
  }
}
