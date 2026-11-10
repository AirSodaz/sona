export interface CliStatus {
  installed: boolean;
  inPath: boolean;
  binaryPath: string | null;
  binDir: string | null;
  version: string;
  platform: 'windows' | 'macos' | 'linux';
  installType: string;
  canModifyPath: boolean;
}
