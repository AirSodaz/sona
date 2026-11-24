import {
  AlertCircle,
  AlertTriangle,
  Cloud,
  ExternalLink,
  Pause,
  Play,
  RefreshCw,
  Settings,
  ShieldCheck,
} from 'lucide-react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { syncRuntimeService } from '../services/syncRuntimeService';
import { runSyncNow, setSyncPaused } from '../services/tauri/sync';
import { useUIConfig } from '../stores/configStore';
import { useSyncStatusStore } from '../stores/syncStatusStore';
import type { SyncStatusSnapshot } from '../types/sync';
import '../styles/sync-header-pill.css';

interface SyncHeaderPillProps {
  onOpenSyncSettings: () => void;
}

function formatRelativeTime(
  timestampMs: number | null,
  fallback: string,
  t: (key: string, options?: Record<string, unknown>) => string,
  locale?: string
): string {
  if (!timestampMs) return fallback;
  const diffMs = Date.now() - timestampMs;
  const diffSec = Math.floor(diffMs / 1000);
  if (diffSec < 60) return t('settings.sync.just_now', { defaultValue: 'Just now' });
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60)
    return t('settings.sync.minutes_ago', { defaultValue: '{{count}}m ago', count: diffMin });
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24)
    return t('settings.sync.hours_ago', { defaultValue: '{{count}}h ago', count: diffHours });
  return new Intl.DateTimeFormat(locale || undefined, { month: 'numeric', day: 'numeric' }).format(
    new Date(timestampMs)
  );
}

function getSyncStatusInfo(
  status: SyncStatusSnapshot,
  isLoaded: boolean,
  t: (key: string, options?: Record<string, unknown>) => string,
  locale?: string
): { dotClass: string; tooltipText: string } {
  if (!isLoaded || status.state === 'disabled') {
    return {
      dotClass: 'is-disabled',
      tooltipText: t('settings.sync.tooltip_disabled', {
        defaultValue: 'Cloud Sync: Not configured',
      }),
    };
  }

  if (status.conflictCount > 0) {
    return {
      dotClass: 'is-conflict',
      tooltipText: t('settings.sync.tooltip_conflicts', {
        defaultValue: 'Cloud Sync: {{count}} conflicts found',
        count: status.conflictCount,
      }),
    };
  }

  if (status.state === 'syncing') {
    return {
      dotClass: 'is-syncing',
      tooltipText: t('settings.sync.tooltip_syncing', { defaultValue: 'Cloud Sync: Syncing...' }),
    };
  }

  if (status.state === 'error') {
    return {
      dotClass: 'is-error',
      tooltipText: t('settings.sync.tooltip_error', { defaultValue: 'Cloud Sync: Sync error' }),
    };
  }

  if (status.state === 'paused') {
    return {
      dotClass: 'is-paused',
      tooltipText: t('settings.sync.tooltip_paused', { defaultValue: 'Cloud Sync: Paused' }),
    };
  }

  if (status.state === 'locked') {
    return {
      dotClass: 'is-locked',
      tooltipText: t('settings.sync.tooltip_locked', { defaultValue: 'Cloud Sync: Locked' }),
    };
  }

  if (status.lastSuccessAtMs) {
    const timeStr = formatRelativeTime(
      status.lastSuccessAtMs,
      t('settings.sync.just_now', { defaultValue: 'Just now' }),
      t,
      locale
    );
    return {
      dotClass: 'is-idle',
      tooltipText: t('settings.sync.tooltip_synced_with_time', {
        defaultValue: 'Cloud Sync: Synced ({{time}})',
        time: timeStr,
      }),
    };
  }

  return {
    dotClass: 'is-idle',
    tooltipText: t('settings.sync.tooltip_synced', { defaultValue: 'Cloud Sync: Synced' }),
  };
}

export function SyncHeaderPill({
  onOpenSyncSettings,
}: SyncHeaderPillProps): React.JSX.Element | null {
  const { t, i18n } = useTranslation();
  const enableCloudSync = useUIConfig().enableCloudSync;
  const status = useSyncStatusStore((state) => state.snapshot);
  const isLoaded = useSyncStatusStore((state) => state.isLoaded);
  const setStatus = useSyncStatusStore((state) => state.setSnapshot);
  const setLastRunResult = useSyncStatusStore((state) => state.setLastRunResult);

  const [isOpen, setIsOpen] = React.useState(false);
  const [isActionBusy, setIsActionBusy] = React.useState(false);
  const popoverRef = React.useRef<HTMLDivElement>(null);
  const buttonRef = React.useRef<HTMLButtonElement>(null);

  // Close popover when clicking outside
  React.useEffect(() => {
    if (!isOpen) return;
    const handleClickOutside = (event: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(event.target as Node) &&
        buttonRef.current &&
        !buttonRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsOpen(false);
      }
    };
    window.addEventListener('mousedown', handleClickOutside);
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('mousedown', handleClickOutside);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen]);

  const handleRunNow = async () => {
    setIsActionBusy(true);
    try {
      setLastRunResult(await runSyncNow());
      await syncRuntimeService.refreshStatus();
    } catch {
      // Error is caught and surfaced in status store
    } finally {
      setIsActionBusy(false);
    }
  };

  const handleTogglePause = async () => {
    setIsActionBusy(true);
    try {
      const nextPaused = status.state !== 'paused';
      setStatus(await setSyncPaused(nextPaused));
      if (!nextPaused) {
        syncRuntimeService.requestSync(0);
      }
    } catch {
      // Handled in store
    } finally {
      setIsActionBusy(false);
    }
  };

  const { dotClass, tooltipText } = getSyncStatusInfo(status, isLoaded, t, i18n?.language);

  if (!enableCloudSync) {
    return null;
  }

  return (
    <div className="sync-header-pill-wrapper">
      <button
        ref={buttonRef}
        type="button"
        className="btn btn-icon sync-header-trigger"
        onClick={() => setIsOpen((prev) => !prev)}
        aria-label={tooltipText}
        aria-expanded={isOpen}
        aria-haspopup="dialog"
        data-tooltip={isOpen ? undefined : tooltipText}
        data-tooltip-pos="bottom-left"
      >
        <Cloud size={18} />
        <span className={`sync-status-dot ${dotClass}`} aria-hidden="true" />
      </button>

      {isOpen && (
        <div ref={popoverRef} className="sync-header-popover" role="dialog">
          <div className="sync-popover-header">
            <div className="sync-popover-title-group">
              <Cloud size={16} />
              <strong>{t('settings.sync.title', { defaultValue: 'Cloud Sync' })}</strong>
            </div>
            <span className={`sync-popover-badge is-${status.state}`}>
              {t(`settings.sync.status_${status.state}`, { defaultValue: status.state })}
            </span>
          </div>

          <div className="sync-popover-body">
            {status.state === 'disabled' ? (
              <div className="sync-popover-disabled-state">
                <p>
                  {t('settings.sync.popover_disabled_hint', {
                    defaultValue:
                      'Multi-device sync is not configured yet. Set up WebDAV cloud sync to keep your transcripts safe and in sync across devices.',
                  })}
                </p>
                <button
                  type="button"
                  className="btn btn-primary btn-sm sync-popover-cta"
                  onClick={() => {
                    setIsOpen(false);
                    onOpenSyncSettings();
                  }}
                >
                  <ShieldCheck size={14} />
                  <span>{t('settings.sync.setup_cta', { defaultValue: 'Enable Cloud Sync' })}</span>
                </button>
              </div>
            ) : (
              <>
                {status.conflictCount > 0 && (
                  <div
                    className="sync-popover-conflict-banner"
                    onClick={() => {
                      setIsOpen(false);
                      onOpenSyncSettings();
                    }}
                  >
                    <AlertTriangle size={15} />
                    <span>
                      {t('settings.sync.conflicts_alert_text', {
                        defaultValue: '{{count}} content conflicts need review.',
                        count: status.conflictCount,
                      })}
                    </span>
                    <ExternalLink size={13} />
                  </div>
                )}

                {status.lastError && (
                  <div className="sync-popover-error-banner">
                    <AlertCircle size={14} />
                    <span>{status.lastError.message}</span>
                  </div>
                )}

                <div className="sync-popover-metrics">
                  <div className="sync-popover-metric-row">
                    <span>
                      {t('settings.sync.last_success', { defaultValue: 'Last success' })}:
                    </span>
                    <strong>
                      {formatRelativeTime(
                        status.lastSuccessAtMs,
                        t('settings.sync.never', { defaultValue: 'Never' }),
                        t,
                        i18n?.language
                      )}
                    </strong>
                  </div>
                  <div className="sync-popover-metric-row">
                    <span>{t('settings.sync.preset', { defaultValue: 'Preset scope' })}:</span>
                    <strong>
                      {t(`settings.sync.preset_${status.preset}`, {
                        defaultValue: status.preset ?? '-',
                      })}
                    </strong>
                  </div>
                </div>

                <div className="sync-popover-actions">
                  <button
                    type="button"
                    className="btn btn-primary btn-sm"
                    onClick={() => void handleRunNow()}
                    disabled={
                      isActionBusy || status.state === 'paused' || status.state === 'locked'
                    }
                  >
                    <RefreshCw
                      size={13}
                      className={status.state === 'syncing' ? 'queue-icon-spin' : undefined}
                    />
                    <span>
                      {status.state === 'syncing'
                        ? t('settings.sync.syncing', { defaultValue: 'Syncing...' })
                        : t('settings.sync.run_now', { defaultValue: 'Sync now' })}
                    </span>
                  </button>

                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => void handleTogglePause()}
                    disabled={isActionBusy || status.state === 'locked'}
                  >
                    {status.state === 'paused' ? <Play size={13} /> : <Pause size={13} />}
                    <span>
                      {status.state === 'paused'
                        ? t('settings.sync.resume', { defaultValue: 'Resume' })
                        : t('settings.sync.pause', { defaultValue: 'Pause' })}
                    </span>
                  </button>

                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => {
                      setIsOpen(false);
                      onOpenSyncSettings();
                    }}
                  >
                    <Settings size={13} />
                    <span>{t('settings.sync.manage', { defaultValue: 'Manage' })}</span>
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default SyncHeaderPill;
