import { convertFileSrc } from '@tauri-apps/api/core';

export function convertManagedAudioFileSrc(filePath: string): string {
  return convertFileSrc(filePath);
}
