import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi';
import { getCurrentWebviewWindow, WebviewWindow } from '@tauri-apps/api/webviewWindow';
import {
  availableMonitors,
  currentMonitor,
  getCurrentWindow,
  monitorFromPoint,
  primaryMonitor,
} from '@tauri-apps/api/window';

export type { Monitor } from '@tauri-apps/api/window';

export {
  availableMonitors,
  currentMonitor,
  getCurrentWebviewWindow,
  getCurrentWindow,
  monitorFromPoint,
  PhysicalPosition,
  PhysicalSize,
  primaryMonitor,
  WebviewWindow,
};
