import {
  AlertCircle,
  AlertTriangle,
  BookOpen,
  Check,
  Copy,
  Download,
  FolderOpen,
  RefreshCw,
  Sparkles,
  Terminal,
  Trash2,
  X,
} from 'lucide-react';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { storageLocationService } from '../../../services/storageLocationService';
import { useCliStore } from '../../../stores/cliStore';
import type { CliPathEntrySource, CliPathState, ShellKind } from '../../../types/cli';
import {
  type SegmentedSubTabItem,
  SegmentedSubTabs,
  SettingsItem,
  SettingsSection,
  SettingsTabContainer,
} from '../../settings/SettingsLayout';
import { ToolHeroHeader, type ToolHeroStatusTone } from '../ToolHeroHeader';
import './ToolCliPanel.css';

export function defaultShellForPlatform(platform?: 'windows' | 'macos' | 'linux'): ShellKind {
  if (platform === 'macos') {
    return 'zsh';
  }
  if (platform === 'linux') {
    return 'bash';
  }
  return 'powershell';
}

function shellDisplayName(shell: ShellKind): string {
  switch (shell) {
    case 'powershell':
      return 'PowerShell';
    case 'zsh':
      return 'Zsh';
    case 'bash':
      return 'Bash';
    case 'fish':
      return 'Fish';
  }
}

export function ToolCliPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const {
    status,
    completionStatuses,
    isLoading,
    isTogglingPath,
    removingPathDir,
    busyCompletionShell,
    error,
    pathJustUpdated,
    completionNotice,
    fetchStatus,
    setPathEnabled,
    switchPathToCurrent,
    removePathEntry,
    installCompletion,
    uninstallCompletion,
    resetPathJustUpdated,
    resetCompletionNotice,
    resetError,
  } = useCliStore();

  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [hasManuallySelectedShell, setHasManuallySelectedShell] = useState(false);
  const [selectedShell, setSelectedShell] = useState<ShellKind>(() =>
    defaultShellForPlatform(status?.platform)
  );

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  useEffect(() => {
    if (!hasManuallySelectedShell && status?.platform) {
      setSelectedShell(defaultShellForPlatform(status.platform));
    }
  }, [hasManuallySelectedShell, status?.platform]);

  const copyToClipboard = (text: string, key: string) => {
    void navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => {
      setCopiedKey(null);
    }, 2000);
  };

  const openDirectory = (dirPath: string) => {
    void storageLocationService.openPath(dirPath).catch(() => {});
  };

  const pathState: CliPathState =
    status?.pathState ?? (status?.inPath ? 'active_current' : 'not_in_path');
  const activeEntry = status?.activeEntry ?? null;
  const pathEntries = status?.pathEntries ?? [];
  const isOtherActive = pathState === 'active_other' || pathState === 'shadowed_by_other';

  const statusTone: ToolHeroStatusTone =
    pathState === 'active_current' ? 'active' : isOtherActive ? 'warning' : 'inactive';

  const statusText =
    pathState === 'active_current'
      ? t('tools.cli.status_in_path', { defaultValue: 'In PATH' })
      : pathState === 'active_other'
        ? t('tools.cli.status_other_in_path', { defaultValue: 'Other Env Active' })
        : pathState === 'shadowed_by_other'
          ? t('tools.cli.status_shadowed_in_path', { defaultValue: 'Shadowed in PATH' })
          : t('tools.cli.status_not_in_path', { defaultValue: 'Not in PATH' });

  const formatSourceLabel = (source: CliPathEntrySource): string => {
    switch (source) {
      case 'system_path':
        return t('tools.cli.source_system_path', { defaultValue: 'System PATH' });
      case 'user_path':
        return t('tools.cli.source_user_path', { defaultValue: 'User PATH' });
      case 'unix_symlink':
        return t('tools.cli.source_unix_symlink', { defaultValue: 'User Symlink' });
      case 'unix_path':
        return t('tools.cli.source_unix_path', { defaultValue: 'PATH Entry' });
    }
  };

  const shellTabItems: SegmentedSubTabItem<ShellKind>[] = [
    { value: 'powershell', label: 'PowerShell' },
    { value: 'zsh', label: 'Zsh' },
    { value: 'bash', label: 'Bash' },
    { value: 'fish', label: 'Fish' },
  ];

  const isWindows = (status?.platform ?? 'windows') === 'windows';
  const fallbackCompletions: Record<
    ShellKind,
    { desc: string; scriptCmd: string; profileLine: string | null }
  > = {
    powershell: {
      desc: t('tools.cli.shell_ps_desc', {
        defaultValue:
          'Write a standalone completion script and load it with a single line in your PowerShell profile ($PROFILE):',
      }),
      scriptCmd: isWindows
        ? 'New-Item -ItemType Directory -Force "$HOME\\.config\\sona\\completions" | Out-Null; sona-cli completion powershell | Out-File -Encoding utf8 "$HOME\\.config\\sona\\completions\\sona-cli.ps1"'
        : 'mkdir -p ~/.config/sona/completions && sona-cli completion powershell > ~/.config/sona/completions/sona-cli.ps1',
      profileLine: isWindows
        ? 'if (Test-Path "$HOME\\.config\\sona\\completions\\sona-cli.ps1") { . "$HOME\\.config\\sona\\completions\\sona-cli.ps1" }'
        : 'if (Test-Path "$HOME/.config/sona/completions/sona-cli.ps1") { . "$HOME/.config/sona/completions/sona-cli.ps1" }',
    },
    zsh: {
      desc: t('tools.cli.shell_zsh_desc', {
        defaultValue: 'Generate completion script into your Zsh fpath directory:',
      }),
      scriptCmd: 'mkdir -p ~/.zfunc && sona-cli completion zsh > ~/.zfunc/_sona-cli',
      profileLine: 'fpath=(~/.zfunc $fpath); autoload -Uz compinit && compinit',
    },
    bash: {
      desc: t('tools.cli.shell_bash_desc', {
        defaultValue: 'Save completion script to user bash-completion directory:',
      }),
      scriptCmd:
        'mkdir -p ~/.local/share/bash-completion/completions && sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli',
      profileLine: null,
    },
    fish: {
      desc: t('tools.cli.shell_fish_desc', {
        defaultValue: 'Save completion script into Fish completions configuration:',
      }),
      scriptCmd:
        'mkdir -p ~/.config/fish/completions && sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish',
      profileLine: null,
    },
  };

  const currentCompletion = completionStatuses[selectedShell];
  const manualScriptCmd =
    currentCompletion?.manualScriptCmd ?? fallbackCompletions[selectedShell].scriptCmd;
  const manualProfileLine =
    currentCompletion !== undefined
      ? currentCompletion.manualProfileLine
      : fallbackCompletions[selectedShell].profileLine;
  const completionState = currentCompletion?.state ?? 'not_installed';
  const isCompletionBusy = busyCompletionShell === selectedShell;

  const cheatSheetCommands = [
    {
      id: 'transcribe',
      tag: 'batch',
      title: t('tools.cli.cmd_transcribe_title', { defaultValue: 'Batch Transcription' }),
      desc: t('tools.cli.cmd_transcribe_desc', {
        defaultValue: 'Transcribe audio/video files and export subtitles.',
      }),
      cmd: 'sona-cli transcribe ./audio.wav -m whisper-turbo -o ./subtitle.srt',
    },
    {
      id: 'live',
      tag: 'realtime',
      title: t('tools.cli.cmd_live_title', { defaultValue: 'Live Microphone Recognition' }),
      desc: t('tools.cli.cmd_live_desc', {
        defaultValue: 'Stream real-time microphone transcription to console.',
      }),
      cmd: 'sona-cli live -m sensevoice',
    },
    {
      id: 'pipe',
      tag: 'stream',
      title: t('tools.cli.cmd_pipe_title', { defaultValue: 'Audio Pipe Stream' }),
      desc: t('tools.cli.cmd_pipe_desc', {
        defaultValue: 'Pipe raw PCM audio into sona-cli via stdin.',
      }),
      cmd: 'ffmpeg -i video.mp4 -f s16le -ac 1 -ar 16000 - | sona-cli live --input stdin',
    },
    {
      id: 'models',
      tag: 'manage',
      title: t('tools.cli.cmd_models_title', { defaultValue: 'Model Management' }),
      desc: t('tools.cli.cmd_models_desc', {
        defaultValue: 'Inspect installed models and download new ones.',
      }),
      cmd: 'sona-cli models list',
    },
    {
      id: 'online',
      tag: 'cloud',
      title: t('tools.cli.cmd_online_title', { defaultValue: 'Cloud ASR Transcription' }),
      desc: t('tools.cli.cmd_online_desc', {
        defaultValue: 'Transcribe using cloud providers (e.g. Groq Whisper).',
      }),
      cmd: 'sona-cli transcribe ./meeting.m4a --online-provider groq-whisper',
    },
  ];

  const pathStatusHint =
    pathState === 'active_current'
      ? t('tools.cli.path_active_hint', {
          defaultValue: 'Configured in system PATH and ready to run.',
        })
      : pathState === 'active_other'
        ? t('tools.cli.path_other_hint', {
            defaultValue: 'Another environment ({{path}}) takes precedence in terminal PATH.',
            path: activeEntry?.pathDir ?? '',
          })
        : pathState === 'shadowed_by_other'
          ? t('tools.cli.path_shadowed_hint', {
              defaultValue: 'In PATH, but shadowed by {{path}} ahead of it.',
              path: activeEntry?.pathDir ?? '',
            })
          : t('tools.cli.path_inactive_hint', {
              defaultValue: 'Enable to run sona-cli and sona-mcp directly from any terminal.',
            });

  const completionBadgeClass =
    completionState === 'installed'
      ? 'badge-completed'
      : completionState === 'outdated' || completionState === 'partial'
        ? 'badge-pending'
        : 'badge-subtle';

  const completionBadgeLabel =
    completionState === 'installed'
      ? t('tools.cli.completion_state_installed', { defaultValue: 'Installed' })
      : completionState === 'outdated'
        ? t('tools.cli.completion_state_outdated', { defaultValue: 'Update Available' })
        : completionState === 'partial'
          ? t('tools.cli.completion_state_partial', { defaultValue: 'Incomplete Setup' })
          : t('tools.cli.completion_state_not_installed', { defaultValue: 'Not Installed' });

  const completionActionLabel =
    completionState === 'outdated'
      ? t('tools.cli.completion_update', {
          defaultValue: 'Update to v{{version}}',
          version: currentCompletion?.currentVersion ?? status?.version ?? '',
        })
      : completionState === 'partial'
        ? t('tools.cli.completion_repair', { defaultValue: 'Repair Setup' })
        : completionState === 'installed'
          ? t('tools.cli.completion_reinstall', { defaultValue: 'Reinstall' })
          : t('tools.cli.completion_install', { defaultValue: 'Install Completion' });

  return (
    <SettingsTabContainer id="tools-panel-cli" ariaLabelledby="tools-tab-cli">
      <ToolHeroHeader
        id="tools-cli-hero"
        icon={<Terminal size={24} />}
        title={t('tools.cli.title', { defaultValue: 'Sona CLI' })}
        description={t('tools.cli.description', {
          defaultValue:
            'Command line tool for audio transcription, real-time recognition, and automation.',
        })}
        statusText={statusText}
        statusTone={statusTone}
        enabled={pathState === 'active_current'}
        disabled={
          isTogglingPath ||
          !status?.canModifyPath ||
          (isOtherActive && Boolean(status?.blockedBySystemPath))
        }
        onToggle={(checked) => {
          if (checked && isOtherActive) {
            void switchPathToCurrent();
          } else {
            void setPathEnabled(checked);
          }
        }}
        switchAriaLabel={t('tools.cli.path_toggle_aria', {
          defaultValue: 'Toggle Sona CLI in PATH',
        })}
        actions={
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => void fetchStatus()}
            disabled={isLoading}
            title={t('common.refresh', { defaultValue: 'Refresh' })}
            aria-label={t('common.refresh', { defaultValue: 'Refresh' })}
          >
            <RefreshCw size={13} className={isLoading ? 'spin' : ''} />
            <span>{t('common.refresh', { defaultValue: 'Refresh' })}</span>
          </button>
        }
      />

      <div className="tools-panel-body">
        {error && (
          <div className="tool-cli-alert-banner alert-error" role="alert">
            <div className="tool-cli-alert-content">
              <AlertCircle size={16} className="tool-cli-alert-icon" />
              <div className="tool-cli-alert-message">
                <strong>
                  {t('common.error', {
                    defaultValue: 'Error',
                  })}
                  :
                </strong>{' '}
                {error}
              </div>
            </div>
            <button
              type="button"
              className="btn-icon-sm"
              onClick={resetError}
              aria-label={t('common.dismiss', { defaultValue: 'Dismiss' })}
            >
              <X size={14} />
            </button>
          </div>
        )}

        {pathJustUpdated && (
          <div className="tool-cli-alert-banner" role="alert">
            <div className="tool-cli-alert-content">
              <AlertCircle size={16} className="tool-cli-alert-icon" />
              <div className="tool-cli-alert-message">
                <strong>
                  {t('tools.cli.path_updated_title', {
                    defaultValue: 'Environment Variable Updated',
                  })}
                  :
                </strong>{' '}
                {t('tools.cli.path_restart_tip', {
                  defaultValue:
                    'Please restart your terminal or code editor to load the updated PATH.',
                })}
              </div>
            </div>
            <button
              type="button"
              className="btn-icon-sm"
              onClick={resetPathJustUpdated}
              aria-label={t('common.dismiss', { defaultValue: 'Dismiss' })}
            >
              <X size={14} />
            </button>
          </div>
        )}

        {/* 1. System PATH Integration */}
        <SettingsSection
          id="tools-cli-path"
          title={t('tools.cli.path_management_title', { defaultValue: 'System PATH Integration' })}
          description={t('tools.cli.path_management_desc', {
            defaultValue: 'Add CLI and MCP binaries to system PATH to run them from any terminal.',
          })}
          icon={<Terminal size={18} />}
        >
          {isOtherActive && activeEntry && (
            <div className="tool-cli-inline-warning" role="status">
              <AlertTriangle size={16} className="tool-cli-alert-icon" />
              <div className="tool-cli-alert-message">
                <strong>
                  {t('tools.cli.multi_env_warning_title', {
                    defaultValue: 'Another Sona Environment Is Active in Terminals',
                  })}
                </strong>
                <div>
                  {t('tools.cli.multi_env_warning_body', {
                    defaultValue:
                      'Terminals currently resolve sona-cli from {{activeDir}} ({{version}}), rather than this running app.',
                    activeDir: activeEntry.pathDir,
                    version: activeEntry.version ? `v${activeEntry.version}` : '?',
                    currentDir: status?.binDir || status?.binaryPath || '-',
                  })}
                </div>
                {status?.blockedBySystemPath && (
                  <div className="tool-cli-alert-subtext">
                    {t('tools.cli.system_path_blocked_tip', {
                      defaultValue:
                        'A system-level PATH entry takes precedence and requires administrator privileges to modify.',
                    })}
                  </div>
                )}
              </div>
            </div>
          )}

          <SettingsItem
            title={t('tools.cli.path_state_title', { defaultValue: 'System PATH Status' })}
            hint={pathStatusHint}
          >
            <div className="tool-cli-actions-row">
              <span
                className={`badge ${
                  pathState === 'active_current' ? 'badge-completed' : 'badge-pending'
                }`}
              >
                <span
                  className={`tool-cli-status-dot ${
                    pathState === 'active_current' ? 'active' : isOtherActive ? 'warning' : ''
                  }`}
                />
                {statusText}
              </span>
              {status?.canModifyPath ? (
                pathState === 'active_current' ? (
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    disabled={isTogglingPath}
                    onClick={() => void setPathEnabled(false)}
                  >
                    {isTogglingPath ? (
                      <RefreshCw size={13} className="spin" />
                    ) : (
                      t('tools.cli.remove_from_path', { defaultValue: 'Remove from PATH' })
                    )}
                  </button>
                ) : isOtherActive ? (
                  status.blockedBySystemPath ? (
                    <span className="badge badge-subtle">
                      {t('tools.cli.system_managed_path', { defaultValue: 'System Managed' })}
                    </span>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-primary btn-sm"
                      disabled={isTogglingPath}
                      onClick={() => void switchPathToCurrent()}
                    >
                      {isTogglingPath ? (
                        <RefreshCw size={13} className="spin" />
                      ) : (
                        t('tools.cli.switch_to_current', {
                          defaultValue: 'Switch PATH to Current App',
                        })
                      )}
                    </button>
                  )
                ) : (
                  <button
                    type="button"
                    className="btn btn-primary btn-sm"
                    disabled={isTogglingPath}
                    onClick={() => void setPathEnabled(true)}
                  >
                    {isTogglingPath ? (
                      <RefreshCw size={13} className="spin" />
                    ) : (
                      t('tools.cli.add_to_path', { defaultValue: 'Add to System PATH' })
                    )}
                  </button>
                )
              ) : (
                <span className="badge badge-subtle">
                  {t('tools.cli.system_managed_path', { defaultValue: 'System Managed' })}
                </span>
              )}
            </div>
          </SettingsItem>

          <SettingsItem
            title={
              activeEntry && !activeEntry.isCurrentApp
                ? t('tools.cli.current_bin_directory', {
                    defaultValue: 'Current App Binary Directory',
                  })
                : t('tools.cli.bin_directory', { defaultValue: 'App Binary Directory' })
            }
            hint={t('tools.cli.bin_dir_desc', {
              defaultValue: 'Directory containing sona-cli and sona-mcp binaries.',
            })}
          >
            <div className="tool-cli-path-display-group">
              <div
                className="tool-cli-path-tooltip-wrapper"
                data-tooltip={status?.binDir || undefined}
                data-tooltip-pos="top"
                data-tooltip-multiline
              >
                <code
                  className={`settings-code-inline ${status?.binDir ? 'tool-cli-path-code' : ''}`}
                >
                  {status?.binDir || t('tools.cli.not_located', { defaultValue: 'Not located' })}
                </code>
              </div>
              {status?.binDir && (
                <>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => status.binDir && copyToClipboard(status.binDir, 'binDir')}
                  >
                    {copiedKey === 'binDir' ? <Check size={13} /> : <Copy size={13} />}
                    <span>
                      {copiedKey === 'binDir'
                        ? t('common.copied', { defaultValue: 'Copied' })
                        : t('common.copy', { defaultValue: 'Copy' })}
                    </span>
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm tool-cli-btn-icon"
                    onClick={() => status.binDir && openDirectory(status.binDir)}
                    title={t('tools.cli.open_folder', { defaultValue: 'Open Folder' })}
                    aria-label={t('tools.cli.open_folder', { defaultValue: 'Open Folder' })}
                  >
                    <FolderOpen size={13} />
                  </button>
                </>
              )}
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('tools.cli.target_binary', { defaultValue: 'Target Executable' })}
            hint={status?.binaryPath || t('tools.cli.not_located', { defaultValue: 'Not located' })}
          >
            <div className="tool-cli-actions-row">
              <span className="badge badge-subtle">{status?.installType || '-'}</span>
              <span className="badge badge-subtle">v{status?.version || '0.10.5'}</span>
            </div>
          </SettingsItem>

          {(pathEntries.length > 1 || isOtherActive) && (
            <SettingsItem
              layout="vertical"
              title={t('tools.cli.detected_environments', {
                defaultValue: 'Detected Environments in PATH',
              })}
              hint={t('tools.cli.detected_environments_desc', {
                defaultValue: 'Listed in PATH priority order. Entry #1 is invoked in terminals.',
              })}
            >
              <div className="tool-cli-env-list">
                {pathEntries.map((entry, idx) => (
                  <div
                    key={`${entry.source}:${entry.pathDir}`}
                    className={`tool-cli-env-card ${entry.isActive ? 'is-active' : ''}`}
                  >
                    <div className="tool-cli-env-card-main">
                      <div className="tool-cli-env-badges">
                        <span className="tool-cli-env-rank">#{idx + 1}</span>
                        <span
                          className={`badge ${entry.isActive ? 'badge-completed' : 'badge-subtle'}`}
                        >
                          {entry.isActive
                            ? t('tools.cli.env_badge_active', { defaultValue: 'Active (#1)' })
                            : t('tools.cli.env_badge_shadowed', { defaultValue: 'Shadowed' })}
                        </span>
                        {entry.isCurrentApp && (
                          <span className="badge badge-processing">
                            {t('tools.cli.env_badge_current', { defaultValue: 'Current App' })}
                          </span>
                        )}
                        {entry.isBroken && (
                          <span className="badge badge-failed">
                            {t('tools.cli.env_badge_broken', {
                              defaultValue: 'Broken Shim / Link',
                            })}
                          </span>
                        )}
                        <span className="badge badge-subtle">
                          {formatSourceLabel(entry.source)}
                        </span>
                        {entry.version && (
                          <span className="badge badge-subtle">v{entry.version}</span>
                        )}
                      </div>
                      <code className="tool-cli-env-path">{entry.pathDir}</code>
                      {entry.resolvedBinaryPath !== entry.entryBinaryPath && (
                        <div className="tool-cli-env-resolved">→ {entry.resolvedBinaryPath}</div>
                      )}
                    </div>
                    <div className="tool-cli-env-actions">
                      <button
                        type="button"
                        className="btn btn-icon btn-secondary-soft btn-sm"
                        onClick={() => openDirectory(entry.pathDir)}
                        title={t('tools.cli.open_folder', { defaultValue: 'Open Folder' })}
                        aria-label={t('tools.cli.open_folder', { defaultValue: 'Open Folder' })}
                      >
                        <FolderOpen size={13} />
                      </button>
                      {entry.canRemove && (
                        <button
                          type="button"
                          className="btn btn-icon btn-danger-soft btn-sm"
                          disabled={removingPathDir === entry.pathDir}
                          onClick={() => void removePathEntry(entry.pathDir)}
                          title={t('tools.cli.remove_entry', { defaultValue: 'Remove' })}
                          aria-label={t('tools.cli.remove_entry', { defaultValue: 'Remove' })}
                        >
                          {removingPathDir === entry.pathDir ? (
                            <RefreshCw size={13} className="spin" />
                          ) : (
                            <Trash2 size={13} />
                          )}
                        </button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </SettingsItem>
          )}
        </SettingsSection>

        {/* 2. Shell Auto-Completion */}
        <SettingsSection
          id="tools-cli-completions"
          title={t('tools.cli.completions_title', { defaultValue: 'Shell Auto-Completion' })}
          description={t('tools.cli.completions_desc', {
            defaultValue: 'Enable tab auto-completion for commands and arguments.',
          })}
          icon={<Sparkles size={18} />}
        >
          {completionNotice && (
            <div className="tool-cli-alert-banner alert-info" role="status">
              <div className="tool-cli-alert-content">
                <Check size={16} className="tool-cli-alert-icon" />
                <div className="tool-cli-alert-message">
                  <strong>
                    {t('tools.cli.completion_updated_title', {
                      defaultValue: 'Shell Completion Updated',
                    })}
                    :
                  </strong>{' '}
                  {t('tools.cli.completion_restart_tip', {
                    defaultValue:
                      'Restart or reload your {{shell}} session to activate completions.',
                    shell: shellDisplayName(completionNotice),
                  })}
                </div>
              </div>
              <button
                type="button"
                className="btn-icon-sm"
                onClick={resetCompletionNotice}
                aria-label={t('common.dismiss', { defaultValue: 'Dismiss' })}
              >
                <X size={14} />
              </button>
            </div>
          )}

          {currentCompletion?.hasLegacyInlineBlock && (
            <div className="tool-cli-inline-warning" role="status">
              <AlertTriangle size={16} className="tool-cli-alert-icon" />
              <div className="tool-cli-alert-message">
                <strong>
                  {t('tools.cli.completion_legacy_warning_title', {
                    defaultValue: 'Legacy Inline Completion Detected in $PROFILE',
                  })}
                </strong>
                <div>
                  {t('tools.cli.completion_legacy_warning_body', {
                    defaultValue:
                      'Your PowerShell profile still contains legacy inline completion code. Removing it prevents duplicate completions and speeds up shell startup.',
                  })}
                </div>
              </div>
            </div>
          )}

          <SettingsItem
            layout="vertical"
            title={t('tools.cli.shell_picker_title', { defaultValue: 'Terminal Shell' })}
          >
            <div className="tool-cli-completion-content">
              <SegmentedSubTabs
                items={shellTabItems}
                activeTab={selectedShell}
                onChange={(val) => {
                  setHasManuallySelectedShell(true);
                  setSelectedShell(val as ShellKind);
                }}
                size="sm"
              />

              <div className="tool-cli-managed-card">
                <div className="tool-cli-managed-header">
                  <div className="tool-cli-managed-status">
                    <span className="tool-cli-managed-title">
                      {t('tools.cli.completion_status_title', {
                        defaultValue: 'Managed Setup',
                      })}
                    </span>
                    <span className={`badge ${completionBadgeClass}`}>{completionBadgeLabel}</span>
                    {currentCompletion?.installedVersion && (
                      <span className="badge badge-subtle">
                        v{currentCompletion.installedVersion}
                        {completionState === 'outdated' &&
                          ` → v${currentCompletion.currentVersion}`}
                      </span>
                    )}
                  </div>
                  <div className="tool-cli-actions-row">
                    <button
                      type="button"
                      className={`btn ${
                        completionState === 'installed' ? 'btn-secondary' : 'btn-primary'
                      } btn-sm`}
                      disabled={isCompletionBusy}
                      onClick={() => void installCompletion(selectedShell)}
                    >
                      {isCompletionBusy ? (
                        <RefreshCw size={13} className="spin" />
                      ) : (
                        <Download size={13} />
                      )}
                      <span>{completionActionLabel}</span>
                    </button>
                    {completionState !== 'not_installed' && (
                      <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        disabled={isCompletionBusy}
                        onClick={() => void uninstallCompletion(selectedShell)}
                      >
                        <Trash2 size={13} />
                        <span>
                          {t('tools.cli.completion_uninstall', { defaultValue: 'Uninstall' })}
                        </span>
                      </button>
                    )}
                  </div>
                </div>

                {currentCompletion && (
                  <div className="tool-cli-managed-meta">
                    <div className="tool-cli-meta-row">
                      <span className="tool-cli-meta-label">
                        {t('tools.cli.completion_script_file', {
                          defaultValue: 'Completion Script',
                        })}
                      </span>
                      <div className="tool-cli-meta-value">
                        <code className="settings-code-inline">{currentCompletion.scriptPath}</code>
                        <button
                          type="button"
                          className="btn btn-secondary btn-sm"
                          onClick={() =>
                            copyToClipboard(
                              currentCompletion.scriptPath,
                              `script_path_${selectedShell}`
                            )
                          }
                        >
                          {copiedKey === `script_path_${selectedShell}` ? (
                            <Check size={13} />
                          ) : (
                            <Copy size={13} />
                          )}
                        </button>
                      </div>
                    </div>

                    <div className="tool-cli-meta-row">
                      <span className="tool-cli-meta-label">
                        {t('tools.cli.completion_profile_file', {
                          defaultValue: 'Profile Configuration',
                        })}
                      </span>
                      <div className="tool-cli-meta-value">
                        {currentCompletion.profilePaths.length > 0 ? (
                          currentCompletion.profilePaths.map((profilePath) => (
                            <code key={profilePath} className="settings-code-inline">
                              {profilePath}
                            </code>
                          ))
                        ) : (
                          <span className="tool-cli-meta-muted">
                            {t('tools.cli.completion_no_profile_needed', {
                              defaultValue: 'Auto-loaded by shell; no profile edits required.',
                            })}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                )}
              </div>

              <div className="tool-cli-manual-header">
                {t('tools.cli.manual_setup_title', { defaultValue: 'Manual Setup Commands' })}
              </div>

              <div className="tool-cli-terminal-card snippet-compact">
                <div className="tool-cli-terminal-body">
                  <span className="tool-cli-prompt" aria-hidden="true">
                    {selectedShell === 'powershell' ? 'PS >' : '$'}
                  </span>
                  <code className="tool-cli-command">{manualScriptCmd}</code>
                  <button
                    type="button"
                    className="btn btn-icon btn-sm tool-cli-profile-copy"
                    onClick={() => copyToClipboard(manualScriptCmd, `shell_${selectedShell}`)}
                    aria-label={
                      copiedKey === `shell_${selectedShell}`
                        ? t('common.copied', { defaultValue: 'Copied' })
                        : t('common.copy', { defaultValue: 'Copy' })
                    }
                  >
                    {copiedKey === `shell_${selectedShell}` ? (
                      <Check size={14} />
                    ) : (
                      <Copy size={14} />
                    )}
                  </button>
                </div>
              </div>

              {manualProfileLine && (
                <>
                  <div className="tool-cli-manual-hint">
                    {t('tools.cli.manual_profile_line_hint', {
                      defaultValue: 'Add this loader line to your shell profile:',
                    })}
                  </div>
                  <div className="tool-cli-terminal-card snippet-compact">
                    <div className="tool-cli-terminal-body">
                      <span className="tool-cli-prompt" aria-hidden="true">
                        {selectedShell === 'powershell' ? 'PS >' : '$'}
                      </span>
                      <code className="tool-cli-command">{manualProfileLine}</code>
                      <button
                        type="button"
                        className="btn btn-icon btn-sm tool-cli-profile-copy"
                        onClick={() =>
                          copyToClipboard(manualProfileLine, `profile_line_${selectedShell}`)
                        }
                        aria-label={
                          copiedKey === `profile_line_${selectedShell}`
                            ? t('common.copied', { defaultValue: 'Copied' })
                            : t('common.copy', { defaultValue: 'Copy' })
                        }
                      >
                        {copiedKey === `profile_line_${selectedShell}` ? (
                          <Check size={14} />
                        ) : (
                          <Copy size={14} />
                        )}
                      </button>
                    </div>
                  </div>
                </>
              )}
            </div>
          </SettingsItem>
        </SettingsSection>

        {/* 3. Command Reference */}
        <SettingsSection
          id="tools-cli-reference"
          title={t('tools.cli.quick_start_title', {
            defaultValue: 'Quick Reference',
          })}
          description={t('tools.cli.quick_start_desc', {
            defaultValue: 'Common command patterns for audio transcription and live streams.',
          })}
          icon={<BookOpen size={18} />}
        >
          {cheatSheetCommands.map((item) => (
            <SettingsItem
              key={item.id}
              layout="vertical"
              title={
                <span className="tool-cli-ref-header">
                  <span>{item.title}</span>
                  <span className="tool-cli-cmd-pill">{item.tag}</span>
                </span>
              }
              hint={item.desc}
            >
              <div className="tool-cli-terminal-card snippet-compact">
                <div className="tool-cli-terminal-body">
                  <span className="tool-cli-prompt" aria-hidden="true">
                    $
                  </span>
                  <code className="tool-cli-command">{item.cmd}</code>
                  <button
                    type="button"
                    className="btn btn-icon btn-sm tool-cli-terminal-copy"
                    onClick={() => copyToClipboard(item.cmd, item.id)}
                    aria-label={
                      copiedKey === item.id
                        ? t('common.copied', { defaultValue: 'Copied' })
                        : t('common.copy', { defaultValue: 'Copy' })
                    }
                  >
                    {copiedKey === item.id ? <Check size={14} /> : <Copy size={14} />}
                  </button>
                </div>
              </div>
            </SettingsItem>
          ))}
        </SettingsSection>
      </div>
    </SettingsTabContainer>
  );
}

export default ToolCliPanel;
