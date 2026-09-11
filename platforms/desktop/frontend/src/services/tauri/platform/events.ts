import { emit, emitTo, listen } from '@tauri-apps/api/event';

export type {
  Event,
  EventCallback,
  EventName,
  EventTarget,
  UnlistenFn,
} from '@tauri-apps/api/event';

export { emit, emitTo, listen };
