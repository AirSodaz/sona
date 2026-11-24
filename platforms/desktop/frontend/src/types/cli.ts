export type CliPathState = 'not_in_path' | 'active_current' | 'active_other' | 'shadowed_by_other';

export type CliPathEntrySource = 'system_path' | 'user_path' | 'unix_symlink' | 'unix_path';

export interface CliPathEntry {
  pathDir: string;
  entryBinaryPath: string;
  resolvedBinaryPath: string;
  version: string | null;
  source: CliPathEntrySource;
  isActive: boolean;
  isCurrentApp: boolean;
  canRemove: boolean;
  isBroken: boolean;
}

export interface CliStatus {
  installed: boolean;
  inPath: boolean;
  pathState: CliPathState;
  binaryPath: string | null;
  binDir: string | null;
  version: string;
  platform: 'windows' | 'macos' | 'linux';
  installType: string;
  canModifyPath: boolean;
  blockedBySystemPath: boolean;
  activeEntry: CliPathEntry | null;
  pathEntries: CliPathEntry[];
}

export type ShellKind = 'powershell' | 'zsh' | 'bash' | 'fish';

export type CompletionInstallState = 'not_installed' | 'installed' | 'outdated' | 'partial';

export interface ShellCompletionStatus {
  shell: ShellKind;
  state: CompletionInstallState;
  scriptPath: string;
  profilePaths: string[];
  installedVersion: string | null;
  currentVersion: string;
  profileManaged: boolean;
  hasLegacyInlineBlock: boolean;
  manualScriptCmd: string;
  manualProfileLine: string | null;
}
