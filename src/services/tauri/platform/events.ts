import { emitTo, listen } from '@tauri-apps/api/event';

export type {
  Event,
  EventCallback,
  EventName,
  EventTarget,
  UnlistenFn,
} from '@tauri-apps/api/event';

export { emitTo, listen };
