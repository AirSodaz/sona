import {
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  ExternalLink,
  HelpCircle,
  RefreshCw,
} from 'lucide-react';
import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '../../services/tauri/platform/opener';
import { getCurrentPlatform } from '../../services/voiceTyping/voiceTypingContext';
import type { HostPlatform } from '../../types/config';
import { logger } from '../../utils/logger';
import './FfmpegInstallGuide.css';

export interface FfmpegInstallGuideProps {
  /** Validity status of FFmpeg: true = ready, false = missing/invalid, null = pending/unknown */
  isValid: boolean | null;
  /** Callback to trigger re-check */
  onRecheck: () => Promise<boolean | undefined>;
  /** Whether re-check or status probe is currently running */
  isRechecking?: boolean;
  /** Explicit platform override (useful for testing); defaults to runtime detected platform */
  platform?: HostPlatform;
}

interface CommandItem {
  id: string;
  label: string;
  command: string;
}

export function FfmpegInstallGuide({
  isValid,
  onRecheck,
  isRechecking = false,
  platform: propPlatform,
}: FfmpegInstallGuideProps): React.JSX.Element {
  const { t } = useTranslation();
  const currentPlatform = propPlatform ?? getCurrentPlatform();

  // Smart expansion: default expanded if not valid, collapsed if valid
  const [isOpen, setIsOpen] = useState<boolean>(!isValid);
  const prevValidRef = useRef<boolean | null>(isValid);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [recheckStatus, setRecheckStatus] = useState<'success' | 'failed' | null>(null);

  useEffect(() => {
    if (prevValidRef.current !== isValid) {
      if (isValid === true) {
        setIsOpen(false);
      } else if (isValid === false) {
        setIsOpen(true);
      }
      prevValidRef.current = isValid;
    }
  }, [isValid]);

  const handleCopy = async (id: string, text: string) => {
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        setCopiedId(id);
        setTimeout(() => {
          setCopiedId((curr) => (curr === id ? null : curr));
        }, 2000);
      }
    } catch (err) {
      logger.error('Failed to copy command to clipboard:', err);
    }
  };

  const handleManualRecheck = async (e: React.MouseEvent) => {
    e.stopPropagation();
    setRecheckStatus(null);
    try {
      const result = await onRecheck();
      if (result === true || (result === undefined && isValid)) {
        setRecheckStatus('success');
      } else {
        setRecheckStatus('failed');
      }
    } catch {
      setRecheckStatus('failed');
    }
  };

  const handleOpenDownloadLink = async () => {
    const url = 'https://ffmpeg.org/download.html';
    try {
      await openUrl(url);
    } catch (err) {
      logger.error('Failed to open download link via opener:', err);
      if (typeof window !== 'undefined') {
        window.open(url, '_blank', 'noopener,noreferrer');
      }
    }
  };

  const platformBadgeLabel =
    currentPlatform === 'windows' ? 'Windows' : currentPlatform === 'macos' ? 'macOS' : 'Linux';

  const renderPlatformContent = () => {
    let desc = '';
    let commands: CommandItem[] = [];

    switch (currentPlatform) {
      case 'windows':
        desc = t(
          'settings.ffmpeg_guide_windows_desc',
          '在终端（PowerShell 或 CMD）中运行以下任一命令即可自动安装：'
        );
        commands = [
          {
            id: 'win-winget',
            label: 'winget (Windows 10/11 推荐)',
            command: 'winget install Gyan.FFmpeg',
          },
          {
            id: 'win-scoop',
            label: 'Scoop',
            command: 'scoop install ffmpeg',
          },
        ];
        break;

      case 'macos':
        desc = t(
          'settings.ffmpeg_guide_macos_desc',
          '在终端中通过 Homebrew 运行以下命令进行安装：'
        );
        commands = [
          {
            id: 'mac-brew',
            label: 'Homebrew',
            command: 'brew install ffmpeg',
          },
        ];
        break;

      case 'linux':
        desc = t(
          'settings.ffmpeg_guide_linux_desc',
          '在终端中运行适用于您 Linux 发行版的命令进行安装：'
        );
        commands = [
          {
            id: 'linux-apt',
            label: 'Ubuntu / Debian',
            command: 'sudo apt update && sudo apt install -y ffmpeg',
          },
          {
            id: 'linux-dnf',
            label: 'Fedora / RHEL',
            command: 'sudo dnf install -y ffmpeg',
          },
          {
            id: 'linux-pacman',
            label: 'Arch Linux',
            command: 'sudo pacman -S ffmpeg',
          },
        ];
        break;
    }

    return (
      <div className="ffmpeg-install-guide-content">
        <p className="ffmpeg-install-guide-desc">{desc}</p>

        <div className="ffmpeg-install-guide-commands">
          {commands.map((cmd) => {
            const isCopied = copiedId === cmd.id;
            return (
              <div key={cmd.id} className="ffmpeg-install-guide-command-card">
                <span className="ffmpeg-install-guide-command-label">{cmd.label}</span>
                <div className="ffmpeg-install-guide-command-row">
                  <code className="ffmpeg-install-guide-code">{cmd.command}</code>
                  <button
                    type="button"
                    className={`btn btn-secondary btn-sm ffmpeg-install-guide-copy-btn ${
                      isCopied ? 'btn-copied' : ''
                    }`}
                    onClick={() => void handleCopy(cmd.id, cmd.command)}
                    aria-label={`${t('settings.ffmpeg_copy_command', '复制命令')}: ${cmd.command}`}
                  >
                    {isCopied ? (
                      <Check size={14} aria-hidden="true" />
                    ) : (
                      <Copy size={14} aria-hidden="true" />
                    )}
                  </button>
                </div>
              </div>
            );
          })}
        </div>

        <div className="ffmpeg-install-guide-footer">
          <div style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
            <span style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>
              {t('settings.ffmpeg_guide_or_manual', '或前往官网手动下载：')}
            </span>
            <button
              type="button"
              className="ffmpeg-install-guide-manual-link"
              onClick={() => void handleOpenDownloadLink()}
            >
              <span>{t('settings.ffmpeg_guide_official_download', 'FFmpeg 官方下载页面')}</span>
              <ExternalLink size={13} aria-hidden="true" />
            </button>
          </div>

          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={(e) => void handleManualRecheck(e)}
            disabled={isRechecking}
          >
            <RefreshCw
              size={13}
              className={isRechecking ? 'spin-icon' : undefined}
              aria-hidden="true"
            />
            <span>
              {isRechecking
                ? t('settings.ffmpeg_guide_rechecking', '检测中...')
                : t('settings.ffmpeg_guide_recheck', '重新检测')}
            </span>
          </button>
        </div>

        {recheckStatus === 'success' && (
          <div className="ffmpeg-install-guide-status-banner success" role="status">
            <Check size={14} aria-hidden="true" />
            <span>{t('settings.ffmpeg_guide_recheck_success', '已成功检测到 FFmpeg！')}</span>
          </div>
        )}

        {recheckStatus === 'failed' && (
          <div className="ffmpeg-install-guide-status-banner failed" role="alert">
            <span>
              {t(
                'settings.ffmpeg_guide_recheck_failed',
                '仍未检测到 FFmpeg，请检查是否已安装并加入系统 PATH。'
              )}
            </span>
          </div>
        )}
      </div>
    );
  };

  return (
    <div
      className={`ffmpeg-install-guide ${!isValid ? 'missing' : 'ready'}`}
      data-testid="ffmpeg-install-guide"
    >
      <button
        type="button"
        className="ffmpeg-install-guide-header"
        onClick={() => setIsOpen((open) => !open)}
        aria-expanded={isOpen}
      >
        <div className="ffmpeg-install-guide-header-title">
          <HelpCircle size={16} aria-hidden="true" />
          <span>
            {isValid
              ? t('settings.ffmpeg_guide_title_detected', 'FFmpeg 已就绪 (点击查看安装参考)')
              : t('settings.ffmpeg_guide_title', '如何安装 FFmpeg')}
          </span>
          <span className="ffmpeg-install-guide-os-badge">{platformBadgeLabel}</span>
        </div>

        <div className="ffmpeg-install-guide-header-actions">
          <span className="ffmpeg-install-guide-chevron">
            {isOpen ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </span>
        </div>
      </button>

      {isOpen && renderPlatformContent()}
    </div>
  );
}

export default FfmpegInstallGuide;
