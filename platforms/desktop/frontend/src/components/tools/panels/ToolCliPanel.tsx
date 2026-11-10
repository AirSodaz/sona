import { AlertCircle, BookOpen, Check, Copy, RefreshCw, Sparkles, Terminal, X } from 'lucide-react';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useCliStore } from '../../../stores/cliStore';
import {
  type SegmentedSubTabItem,
  SegmentedSubTabs,
  SettingsItem,
  SettingsSection,
  SettingsTabContainer,
} from '../../settings/SettingsLayout';
import { ToolHeroHeader, type ToolHeroStatusTone } from '../ToolHeroHeader';
import './ToolCliPanel.css';

type ShellKind = 'powershell' | 'zsh' | 'bash' | 'fish';

export function ToolCliPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const {
    status,
    isLoading,
    isTogglingPath,
    pathJustUpdated,
    fetchStatus,
    setPathEnabled,
    resetPathJustUpdated,
  } = useCliStore();

  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [selectedShell, setSelectedShell] = useState<ShellKind>('powershell');

  useEffect(() => {
    void fetchStatus();
  }, [fetchStatus]);

  const copyToClipboard = (text: string, key: string) => {
    void navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => {
      setCopiedKey(null);
    }, 2000);
  };

  const inPath = Boolean(status?.inPath);
  const statusTone: ToolHeroStatusTone = inPath ? 'active' : 'inactive';
  const statusText = inPath
    ? t('tools.cli.status_in_path', { defaultValue: 'In PATH' })
    : t('tools.cli.status_not_in_path', { defaultValue: 'Not in PATH' });

  const shellTabItems: SegmentedSubTabItem<ShellKind>[] = [
    { value: 'powershell', label: 'PowerShell' },
    { value: 'zsh', label: 'Zsh' },
    { value: 'bash', label: 'Bash' },
    { value: 'fish', label: 'Fish' },
  ];

  const completionCommands: Record<ShellKind, { desc: string; cmd: string }> = {
    powershell: {
      desc: t('tools.cli.shell_ps_desc', {
        defaultValue: 'Append Sona CLI completion script to your PowerShell profile ($PROFILE):',
      }),
      cmd: 'sona-cli completion powershell | Out-File -Append -Encoding utf8 $PROFILE',
    },
    zsh: {
      desc: t('tools.cli.shell_zsh_desc', {
        defaultValue: 'Generate completion script into your Zsh fpath directory:',
      }),
      cmd: 'sona-cli completion zsh > ~/.zfunc/_sona-cli',
    },
    bash: {
      desc: t('tools.cli.shell_bash_desc', {
        defaultValue: 'Save completion script to user bash-completion directory:',
      }),
      cmd: 'sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli',
    },
    fish: {
      desc: t('tools.cli.shell_fish_desc', {
        defaultValue: 'Save completion script into Fish completions configuration:',
      }),
      cmd: 'sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish',
    },
  };

  const cheatSheetCommands = [
    {
      id: 'transcribe',
      tag: 'batch',
      title: t('tools.cli.cmd_transcribe_title', { defaultValue: 'Batch Transcription' }),
      desc: t('tools.cli.cmd_transcribe_desc', {
        defaultValue:
          'Transcribe an audio or video file with specified ASR model and subtitle export.',
      }),
      cmd: 'sona-cli transcribe ./audio.wav -m whisper-turbo -o ./subtitle.srt',
    },
    {
      id: 'live',
      tag: 'realtime',
      title: t('tools.cli.cmd_live_title', { defaultValue: 'Live Microphone Recognition' }),
      desc: t('tools.cli.cmd_live_desc', {
        defaultValue:
          'Stream real-time transcription from microphone directly into standard output.',
      }),
      cmd: 'sona-cli live -m sensevoice',
    },
    {
      id: 'pipe',
      tag: 'stream',
      title: t('tools.cli.cmd_pipe_title', { defaultValue: 'Audio Pipe Stream' }),
      desc: t('tools.cli.cmd_pipe_desc', {
        defaultValue: 'Pipe raw PCM audio data into sona-cli via stdin for automated processing.',
      }),
      cmd: 'ffmpeg -i video.mp4 -f s16le -ac 1 -ar 16000 - | sona-cli live --input stdin',
    },
    {
      id: 'models',
      tag: 'manage',
      title: t('tools.cli.cmd_models_title', { defaultValue: 'Model Management' }),
      desc: t('tools.cli.cmd_models_desc', {
        defaultValue: 'Inspect installed ASR/VAD/NMT models and download new ones.',
      }),
      cmd: 'sona-cli models list',
    },
    {
      id: 'online',
      tag: 'cloud',
      title: t('tools.cli.cmd_online_title', { defaultValue: 'Cloud ASR Transcription' }),
      desc: t('tools.cli.cmd_online_desc', {
        defaultValue: 'Fast transcription using online cloud providers (e.g. Groq Whisper).',
      }),
      cmd: 'sona-cli transcribe ./meeting.m4a --online-provider groq-whisper',
    },
  ];

  return (
    <SettingsTabContainer id="tools-panel-cli" ariaLabelledby="tools-tab-cli">
      <ToolHeroHeader
        id="tools-cli-hero"
        icon={<Terminal size={24} />}
        title={t('tools.cli.title', { defaultValue: 'Sona CLI' })}
        description={t('tools.cli.description', {
          defaultValue:
            'Standalone command line interface for offline transcription, real-time audio streams, model management, and shell automation.',
        })}
        statusText={statusText}
        statusTone={statusTone}
        enabled={inPath}
        disabled={isTogglingPath || !status?.canModifyPath}
        onToggle={(checked) => void setPathEnabled(checked)}
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
                    'Please restart your terminal, CMD, PowerShell, or code editor to load the updated PATH.',
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
            defaultValue:
              'Add Sona CLI and MCP binaries to your system environment variables so you can run sona-cli from any terminal.',
          })}
          icon={<Terminal size={18} />}
        >
          <SettingsItem
            title={t('tools.cli.path_state_title', { defaultValue: 'System PATH Status' })}
            hint={
              inPath
                ? t('tools.cli.path_active_hint', {
                    defaultValue:
                      'Sona CLI and MCP binaries are configured in your system PATH and ready to run.',
                  })
                : t('tools.cli.path_inactive_hint', {
                    defaultValue:
                      'Enable to allow terminal commands (sona-cli, sona-mcp) to be executed globally.',
                  })
            }
          >
            <div className="tool-cli-actions-row">
              <span className={`badge ${inPath ? 'badge-completed' : 'badge-pending'}`}>
                <span className={`mcp-status-dot ${inPath ? 'active' : ''}`} />
                {statusText}
              </span>
              {status?.canModifyPath ? (
                <button
                  type="button"
                  className={`btn ${inPath ? 'btn-secondary' : 'btn-primary'} btn-sm`}
                  disabled={isTogglingPath}
                  onClick={() => void setPathEnabled(!inPath)}
                >
                  {isTogglingPath ? (
                    <RefreshCw size={13} className="spin" />
                  ) : inPath ? (
                    t('tools.cli.remove_from_path', { defaultValue: 'Remove from PATH' })
                  ) : (
                    t('tools.cli.add_to_path', { defaultValue: 'Add to System PATH' })
                  )}
                </button>
              ) : (
                <span className="badge badge-subtle">
                  {t('tools.cli.system_managed_path', { defaultValue: 'System Managed' })}
                </span>
              )}
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('tools.cli.bin_directory', { defaultValue: 'Dedicated Binary Directory' })}
            hint={t('tools.cli.bin_dir_desc', {
              defaultValue:
                'Isolated bin directory preventing runtime library conflicts and command collisions.',
            })}
          >
            <div className="tool-cli-inline-code-group">
              <code className="settings-code-inline" title={status?.binDir || ''}>
                {status?.binDir || t('tools.cli.not_located', { defaultValue: 'Not located' })}
              </code>
              {status?.binDir && (
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
              )}
            </div>
          </SettingsItem>

          <SettingsItem
            title={t('tools.cli.target_binary', { defaultValue: 'Target Executable' })}
            hint={status?.binaryPath || t('tools.cli.not_located', { defaultValue: 'Not located' })}
          >
            <div className="tool-cli-actions-row">
              <span className="badge badge-subtle">{status?.installType || '-'}</span>
              <span className="badge badge-subtle">v{status?.version || '0.10.3'}</span>
            </div>
          </SettingsItem>
        </SettingsSection>

        {/* 2. Shell Auto-Completion */}
        <SettingsSection
          id="tools-cli-completions"
          title={t('tools.cli.completions_title', { defaultValue: 'Shell Auto-Completion' })}
          description={t('tools.cli.completions_desc', {
            defaultValue:
              'Enable tab auto-completion for sona-cli commands, subcommands, and model parameters in your preferred shell.',
          })}
          icon={<Sparkles size={18} />}
        >
          <SettingsItem
            layout="vertical"
            title={t('tools.cli.shell_picker_title', { defaultValue: 'Terminal Shell' })}
            hint={completionCommands[selectedShell].desc}
          >
            <div className="tool-cli-completion-content">
              <SegmentedSubTabs
                items={shellTabItems}
                activeTab={selectedShell}
                onChange={(val) => setSelectedShell(val as ShellKind)}
                size="sm"
              />

              <div className="tool-cli-terminal-card">
                <div className="tool-cli-terminal-header">
                  <div className="tool-cli-terminal-dots" aria-hidden="true">
                    <span className="tool-cli-terminal-dot dot-red" />
                    <span className="tool-cli-terminal-dot dot-yellow" />
                    <span className="tool-cli-terminal-dot dot-green" />
                  </div>
                  <span className="tool-cli-terminal-title">{selectedShell}</span>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() =>
                      copyToClipboard(
                        completionCommands[selectedShell].cmd,
                        `shell_${selectedShell}`
                      )
                    }
                  >
                    {copiedKey === `shell_${selectedShell}` ? (
                      <Check size={13} />
                    ) : (
                      <Copy size={13} />
                    )}
                    <span>
                      {copiedKey === `shell_${selectedShell}`
                        ? t('common.copied', { defaultValue: 'Copied' })
                        : t('common.copy', { defaultValue: 'Copy' })}
                    </span>
                  </button>
                </div>
                <div className="tool-cli-terminal-body">
                  <span className="tool-cli-prompt" aria-hidden="true">
                    {selectedShell === 'powershell' ? 'PS >' : '$'}
                  </span>
                  <code className="tool-cli-command">{completionCommands[selectedShell].cmd}</code>
                </div>
              </div>
            </div>
          </SettingsItem>
        </SettingsSection>

        {/* 3. Command Reference */}
        <SettingsSection
          id="tools-cli-reference"
          title={t('tools.cli.quick_start_title', {
            defaultValue: 'Quick Start & Command Reference',
          })}
          description={t('tools.cli.quick_start_desc', {
            defaultValue:
              'Common everyday command patterns for audio transcription, live streams, and pipeline automation.',
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
                    className="btn btn-secondary btn-sm tool-cli-terminal-copy"
                    onClick={() => copyToClipboard(item.cmd, item.id)}
                  >
                    {copiedKey === item.id ? <Check size={13} /> : <Copy size={13} />}
                    <span>
                      {copiedKey === item.id
                        ? t('common.copied', { defaultValue: 'Copied' })
                        : t('common.copy', { defaultValue: 'Copy' })}
                    </span>
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
