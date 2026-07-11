export const SUPPORTED_MEDIA_EXTENSIONS = [
  '.wav',
  '.mp3',
  '.m4a',
  '.aiff',
  '.flac',
  '.ogg',
  '.wma',
  '.aac',
  '.opus',
  '.amr',
  '.mp4',
  '.webm',
  '.mov',
  '.mkv',
  '.avi',
  '.wmv',
  '.flv',
  '.3gp',
] as const;

export function isSupportedMediaPath(filePath: string): boolean {
  const normalized = filePath.trim().toLowerCase();
  return SUPPORTED_MEDIA_EXTENSIONS.some((extension) => normalized.endsWith(extension));
}
