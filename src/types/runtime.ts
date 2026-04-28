export type RuntimePathStatusKind = 'file' | 'directory' | 'missing' | 'unknown';

export interface RuntimePathStatus {
  path: string;
  kind: RuntimePathStatusKind;
  error?: string | null;
}

export interface RuntimeEnvironmentStatus {
  ffmpegPath: string;
  ffmpegExists: boolean;
  logDirPath: string;
}
