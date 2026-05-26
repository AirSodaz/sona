import { open, save } from '@tauri-apps/plugin-dialog';
import type {
  DialogFilter,
  OpenDialogOptions,
  OpenDialogReturn,
  SaveDialogOptions,
} from '@tauri-apps/plugin-dialog';

export type {
  DialogFilter,
  OpenDialogOptions,
  OpenDialogReturn,
  SaveDialogOptions,
};

export function openDialog<T extends OpenDialogOptions>(options?: T): Promise<OpenDialogReturn<T>> {
  return open(options);
}

export function saveDialog(options?: SaveDialogOptions): Promise<string | null> {
  return save(options);
}
