import {
  AlertCircle,
  AlertTriangle,
  ArrowUpRight,
  Check,
  Clock,
  Copy,
  Download,
  Eye,
  EyeOff,
  KeyRound,
  Layers,
  Link2,
  Lock,
  Pause,
  Play,
  QrCode,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Trash2,
  Unplug,
} from 'lucide-react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { getSyncPairingInfo } from '../../../services/tauri/sync';
import { useDialogStore } from '../../../stores/dialogStore';
import type {
  SyncPairingInfo,
  SyncPresetV1,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
} from '../../../types/sync';
import { Modal } from '../../Modal';
import { SettingsAccordion, SettingsItem, SettingsSection } from '../SettingsLayout';
import { PasswordInput } from './PasswordInput';
import { encodeS3SyncPairingToken, encodeSyncPairingToken } from './syncPairing';

interface SyncConnectedPanelProps {
  busyAction: string | null;
  recoveryKey: string | null;
  status: SyncStatusSnapshot;
  onChangeMasterPassword: (currentPassword: string, nextPassword: string) => Promise<void>;
  onChangePreset: (preset: SyncPresetV1) => Promise<void>;
  onCopyRecoveryKey: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onExportRecoveryKey: () => Promise<void>;
  onGenerateRecoveryKey: () => Promise<void>;
  onLock: () => Promise<void>;
  onRunNow: () => Promise<void>;
  onSetPaused: (paused: boolean) => Promise<void>;
  onUnlock: (request: SyncUnlockRequest) => Promise<void>;
  onUnlockWithRecovery: (request: SyncUnlockRecoveryRequest) => Promise<void>;
  conflictCenterSlot?: React.ReactNode;
  onDeleteRecoveryKey?: () => void;
}

function formatFriendlyTime(
  value: number | null,
  fallback: string,
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  if (!value) return fallback;
  const now = Date.now();
  const diff = now - value;
  if (diff < 0)
    return new Date(value).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  if (diff < 60_000) return t('settings.sync.just_now', { defaultValue: 'Just now' });
  if (diff < 3600_000) {
    const mins = Math.floor(diff / 60_000);
    return t('settings.sync.minutes_ago', { defaultValue: '{{count}}m ago', count: mins });
  }
  if (diff < 86400_000) {
    const hours = Math.floor(diff / 3600_000);
    return t('settings.sync.hours_ago', { defaultValue: '{{count}}h ago', count: hours });
  }
  return new Date(value).toLocaleDateString([], {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
function formatPartiallyMaskedKey(key: string): string {
  if (!key) return '';
  const trimmed = key.trim();
  if (trimmed.length <= 10) {
    return `${trimmed.slice(0, 2)}••••${trimmed.slice(-2)}`;
  }
  const prefix = trimmed.slice(0, 6);
  const suffix = trimmed.slice(-4);
  return `${prefix}••••••••••••••••${suffix}`;
}

export function SyncConnectedPanel({
  busyAction,
  recoveryKey,
  status,
  onChangeMasterPassword,
  onChangePreset,
  onCopyRecoveryKey,
  onDisconnect,
  onExportRecoveryKey,
  onGenerateRecoveryKey,
  onLock,
  onRunNow,
  onSetPaused,
  onUnlock,
  onUnlockWithRecovery,
  conflictCenterSlot,
  onDeleteRecoveryKey,
}: SyncConnectedPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const confirm = useDialogStore((state) => state.confirm);
  const [unlockMode, setUnlockMode] = React.useState<'password' | 'recovery'>('password');
  const [providerPassword, setProviderPassword] = React.useState('');
  const [masterPassword, setMasterPassword] = React.useState('');
  const [recoveryInput, setRecoveryInput] = React.useState('');
  const [userSelectedPreset, setUserSelectedPreset] = React.useState<SyncPresetV1 | null>(null);
  const selectedPreset = userSelectedPreset ?? status.preset ?? 'standard';
  const [currentPassword, setCurrentPassword] = React.useState('');
  const [nextPassword, setNextPassword] = React.useState('');
  const [confirmPassword, setConfirmPassword] = React.useState('');
  const [passwordError, setPasswordError] = React.useState<string | null>(null);
  const [passwordChangeMode, setPasswordChangeMode] = React.useState<'password' | 'recovery'>(
    'password'
  );
  // Pairing Modal state
  const [showPairingModal, setShowPairingModal] = React.useState(false);
  const [copiedToken, setCopiedToken] = React.useState(false);
  const [copiedKey, setCopiedKey] = React.useState(false);
  const [pairingInfo, setPairingInfo] = React.useState<SyncPairingInfo | null>(null);
  const [isKeyRevealed, setIsKeyRevealed] = React.useState(false);

  React.useEffect(() => {
    if (!status.vaultId) {
      setPairingInfo(null);
      return;
    }
    let active = true;
    getSyncPairingInfo()
      .then((info) => {
        if (active && info) {
          setPairingInfo(info);
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [status.vaultId]);
  const serverUrl = pairingInfo?.serverUrl;
  const serverHost = React.useMemo(() => {
    if (!serverUrl) return null;
    try {
      return new URL(serverUrl).host;
    } catch {
      return null;
    }
  }, [serverUrl]);

  const isBusy = busyAction !== null || status.state === 'syncing';
  const handleDeleteRecoveryKey = async () => {
    const confirmed = await confirm(
      t('settings.sync.delete_recovery_key_confirm_message', {
        defaultValue:
          'Delete this recovery key from display? Make sure you have backed it up safely.',
      }),
      {
        title: t('settings.sync.delete_recovery_key_confirm_title', {
          defaultValue: 'Delete Recovery Key',
        }),
        confirmLabel: t('common.delete', { defaultValue: 'Delete' }),
        cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
      }
    );
    if (confirmed) {
      onDeleteRecoveryKey?.();
    }
  };

  const stateLabel = t(`settings.sync.status_${status.state}`, {
    defaultValue: status.state,
  });
  const retryLabel = formatFriendlyTime(
    status.nextRetryAtMs,
    t('settings.sync.not_scheduled', { defaultValue: 'Not scheduled' }),
    t
  );

  // Generate pairing token for the current vault
  const pairingToken = React.useMemo(() => {
    if (!status.vaultId) return '';
    if (pairingInfo?.providerId === 's3' || pairingInfo?.endpoint) {
      return encodeS3SyncPairingToken(
        {
          endpoint: pairingInfo?.endpoint || '',
          region: pairingInfo?.region || 'us-east-1',
          bucket: pairingInfo?.bucket || '',
          remoteRoot: pairingInfo?.remoteRoot || 'sona',
          accessKeyId: pairingInfo?.accessKeyId || '',
          secretAccessKey: '',
          forcePathStyle: pairingInfo?.forcePathStyle ?? false,
        },
        status.vaultId,
        false
      );
    }
    return encodeSyncPairingToken(
      {
        serverUrl: pairingInfo?.serverUrl || '',
        remoteRoot: pairingInfo?.remoteRoot || 'Sona',
        username: pairingInfo?.username || '',
        password: '',
      },
      status.vaultId,
      false
    );
  }, [status.vaultId, pairingInfo]);

  const handleCopyPairingToken = async () => {
    if (pairingToken && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(pairingToken);
      setCopiedToken(true);
      setTimeout(() => setCopiedToken(false), 2000);
    }
  };

  const handleCopyKeyWithFeedback = async () => {
    await onCopyRecoveryKey();
    setCopiedKey(true);
    setTimeout(() => setCopiedKey(false), 2000);
  };

  if (status.state === 'locked') {
    const canUnlock =
      providerPassword.length > 0 &&
      (unlockMode === 'password' ? masterPassword.length > 0 : recoveryInput.length > 0);
    return (
      <form
        className="sync-form sync-unlock-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (unlockMode === 'password') {
            void onUnlock({ providerPassword, masterPassword });
          } else {
            void onUnlockWithRecovery({ providerPassword, recoveryKey: recoveryInput });
          }
        }}
      >
        <div className="sync-status-heading">
          <div className="sync-status-icon is-locked">
            <Lock size={20} />
          </div>
          <div>
            <strong>
              {t('settings.sync.locked_title', { defaultValue: 'Sync vault locked' })}
            </strong>
            <span style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>
              {status.vaultId}
            </span>
          </div>
        </div>

        <div
          className="sync-segmented-control"
          role="tablist"
          aria-label={t('settings.sync.unlock_mode', { defaultValue: 'Unlock method' })}
        >
          <button
            type="button"
            role="tab"
            aria-selected={unlockMode === 'password'}
            className={`sync-segmented-btn${unlockMode === 'password' ? ' is-active' : ''}`}
            onClick={() => setUnlockMode('password')}
          >
            <KeyRound size={15} />
            <span>{t('settings.sync.master_password', { defaultValue: 'Master password' })}</span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={unlockMode === 'recovery'}
            className={`sync-segmented-btn${unlockMode === 'recovery' ? ' is-active' : ''}`}
            onClick={() => setUnlockMode('recovery')}
          >
            <ShieldCheck size={15} />
            <span>{t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}</span>
          </button>
        </div>

        <SettingsItem
          title={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
          hint={t('settings.sync.provider_password_hint', {
            defaultValue: 'Enter your WebDAV application password',
          })}
        >
          <PasswordInput
            id="sync-unlock-provider-password"
            autoComplete="current-password"
            ariaLabel={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
            value={providerPassword}
            onChange={(event) => setProviderPassword(event.target.value)}
            disabled={isBusy}
            style={{ width: '260px' }}
          />
        </SettingsItem>

        {unlockMode === 'password' ? (
          <SettingsItem
            title={t('settings.sync.master_password', { defaultValue: 'Master password' })}
            hint={t('settings.sync.master_password_hint', {
              defaultValue: 'Enter your vault master password',
            })}
          >
            <PasswordInput
              id="sync-unlock-master-password"
              autoComplete="current-password"
              ariaLabel={t('settings.sync.master_password', { defaultValue: 'Master password' })}
              value={masterPassword}
              onChange={(event) => setMasterPassword(event.target.value)}
              disabled={isBusy}
              style={{ width: '260px' }}
            />
          </SettingsItem>
        ) : (
          <SettingsItem
            title={t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}
            hint={t('settings.sync.recovery_key_hint', {
              defaultValue: 'Enter your emergency recovery key',
            })}
          >
            <input
              id="sync-unlock-recovery-key"
              className="settings-input sync-monospace-input"
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={recoveryInput}
              onChange={(event) => setRecoveryInput(event.target.value)}
              disabled={isBusy}
              style={{ width: '260px' }}
            />
          </SettingsItem>
        )}

        <div
          className="sync-status-actions"
          style={{
            justifyContent: 'space-between',
            alignItems: 'center',
            flexWrap: 'wrap',
            gap: '8px',
            paddingTop: '8px',
          }}
        >
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => void onDisconnect()}
            disabled={isBusy}
            style={{ color: 'var(--color-danger, #ef4444)' }}
          >
            <Unplug size={15} />
            <span>{t('settings.sync.disconnect_action', { defaultValue: 'Disconnect' })}</span>
          </button>
          <button type="submit" className="btn btn-primary" disabled={isBusy || !canUnlock}>
            <KeyRound size={16} />
            <span>
              {busyAction === 'unlock'
                ? t('settings.sync.unlocking', { defaultValue: 'Unlocking...' })
                : t('settings.sync.unlock_action', { defaultValue: 'Unlock' })}
            </span>
          </button>
        </div>
      </form>
    );
  }

  return (
    <div className="sync-connected-panel">
      {/* Status Hero Card */}
      <div className={`sync-status-hero-card is-${status.state}`}>
        <div className="sync-hero-top-row">
          <div className="sync-hero-info">
            <div className="sync-hero-status-pill">
              <span className={`sync-status-dot is-${status.state}`} />
              <strong>{stateLabel}</strong>
              <span className="sync-hero-provider-tag">
                {status.providerId === 'webdav' ? 'WebDAV' : status.providerId || 'Encrypted'}
              </span>
              {status.conflictCount > 0 && (
                <span className="sync-conflict-badge">
                  <AlertTriangle size={12} />
                  {t('settings.sync.conflicts_count', {
                    defaultValue: '{{count}} conflicts',
                    count: status.conflictCount,
                  })}
                </span>
              )}
            </div>
            <div className="sync-hero-meta">
              <span>
                {t('settings.sync.vault_identifier_label', { defaultValue: 'Vault: ' })}
                <code>{status.vaultId}</code>
              </span>
              {serverHost && (
                <>
                  <span className="sync-meta-divider">·</span>
                  <span
                    className="sync-hero-server-url"
                    title={pairingInfo?.serverUrl || undefined}
                  >
                    {serverHost}
                  </span>
                </>
              )}
            </div>
          </div>

          <div className="sync-hero-actions">
            <button
              type="button"
              className="btn btn-secondary btn-sm sync-pair-btn"
              onClick={() => setShowPairingModal(true)}
              disabled={isBusy}
            >
              <QrCode size={14} />
              <span>
                {t('settings.sync.pair_device_action', { defaultValue: 'Pair new device' })}
              </span>
            </button>

            <button
              type="button"
              className="btn btn-primary btn-sm sync-run-btn"
              onClick={() => void onRunNow()}
              disabled={isBusy || status.state === 'paused'}
            >
              <RefreshCw
                size={14}
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
              onClick={() => void onSetPaused(status.state !== 'paused')}
              disabled={isBusy}
            >
              {status.state === 'paused' ? <Play size={14} /> : <Pause size={14} />}
              <span>
                {status.state === 'paused'
                  ? t('settings.sync.resume', { defaultValue: 'Resume' })
                  : t('settings.sync.pause', { defaultValue: 'Pause' })}
              </span>
            </button>

            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => void onLock()}
              disabled={isBusy}
              title={t('settings.sync.lock_action', { defaultValue: 'Lock vault' })}
            >
              <Lock size={14} />
              <span>{t('settings.sync.lock_action', { defaultValue: 'Lock' })}</span>
            </button>
          </div>
        </div>

        {/* Status Metrics Grid */}
        <div className="sync-metric-grid">
          <div className="sync-metric-card">
            <span className="sync-metric-label">
              <Clock size={13} />
              {t('settings.sync.last_success', { defaultValue: 'Last sync' })}
            </span>
            <strong className="sync-metric-value">
              {formatFriendlyTime(
                status.lastSuccessAtMs,
                t('settings.sync.never', { defaultValue: 'Never' }),
                t
              )}
            </strong>
          </div>

          <div className="sync-metric-card">
            <span className="sync-metric-label">
              <ArrowUpRight size={13} />
              {t('settings.sync.pending', { defaultValue: 'Pending upload' })}
            </span>
            <strong
              className={`sync-metric-value ${status.pendingOperationCount > 0 ? 'is-active' : ''}`}
            >
              {status.pendingOperationCount}
            </strong>
          </div>

          <div className="sync-metric-card">
            <span className="sync-metric-label">
              <Layers size={13} />
              {t('settings.sync.preset', { defaultValue: 'Sync scope' })}
            </span>
            <strong className="sync-metric-value">
              {t(`settings.sync.preset_${status.preset}`, { defaultValue: status.preset ?? '-' })}
            </strong>
          </div>

          <div className="sync-metric-card">
            <span className="sync-metric-label">
              <AlertTriangle size={13} />
              {t('settings.sync.conflicts', { defaultValue: 'Conflicts' })}
            </span>
            <strong
              className={`sync-metric-value ${status.conflictCount > 0 ? 'sync-text-warning' : ''}`}
            >
              {status.conflictCount}
            </strong>
          </div>

          {status.state === 'error' && (
            <div className="sync-metric-card sync-status-wide is-error-card">
              <span className="sync-metric-label">
                <AlertCircle size={13} />
                {t('settings.sync.next_retry', { defaultValue: 'Next retry' })}
              </span>
              <strong className="sync-metric-value">{retryLabel}</strong>
            </div>
          )}
        </div>

        {/* Error Banner */}
        {status.lastError && (
          <div className="sync-error-banner" role="alert">
            <div className="sync-error-banner-title">
              <AlertCircle size={15} />
              <strong>{status.lastError.code}</strong>
            </div>
            <span>{status.lastError.message}</span>
          </div>
        )}
      </div>

      {/* Section: Sync Scope */}
      <SettingsSection
        title={t('settings.sync.scope_selector_label', { defaultValue: 'Sync Scope' })}
        description={t('settings.sync.preset_hint', {
          defaultValue: 'Choose which data domains participate in cloud sync.',
        })}
      >
        <SettingsItem
          title={t('settings.sync.preset', { defaultValue: 'Sync preset' })}
          layout="vertical"
        >
          <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div
              className="settings-scenario-cards three-columns"
              style={{ width: '100%', padding: 0, background: 'transparent' }}
            >
              {[
                {
                  id: 'content' as const,
                  label: t('settings.sync.preset_content', { defaultValue: 'Content' }),
                  description: t('settings.sync.scope_transcripts_only', {
                    defaultValue: 'Transcripts & Projects',
                  }),
                },
                {
                  id: 'standard' as const,
                  label: t('settings.sync.preset_standard', { defaultValue: 'Standard' }),
                  description: t('settings.sync.scope_standard_desc', {
                    defaultValue: 'Transcripts, summaries, rules',
                  }),
                  badge: t('common.recommended', { defaultValue: 'Recommended' }),
                },
                {
                  id: 'full' as const,
                  label: t('settings.sync.preset_full', { defaultValue: 'Full' }),
                  description: t('settings.sync.scope_full_desc', {
                    defaultValue: 'Full workspace data',
                  }),
                },
              ].map((p) => {
                const isSelected = selectedPreset === p.id;
                return (
                  <button
                    key={p.id}
                    type="button"
                    className={`settings-scenario-card${isSelected ? ' active' : ''}`}
                    onClick={() => setUserSelectedPreset(p.id)}
                    disabled={isBusy}
                  >
                    <span className="settings-scenario-card-icon">
                      <Layers size={18} />
                    </span>
                    <span className="settings-scenario-card-text">
                      <span className="settings-scenario-card-label">
                        {p.label}
                        {p.badge && (
                          <span className="sync-scope-tag is-badge" style={{ marginLeft: '6px' }}>
                            {p.badge}
                          </span>
                        )}
                      </span>
                      <span className="settings-scenario-card-description">{p.description}</span>
                    </span>
                  </button>
                );
              })}
            </div>
            {selectedPreset !== status.preset && (
              <button
                type="button"
                className="btn btn-primary btn-sm"
                style={{ alignSelf: 'flex-start' }}
                onClick={() => {
                  void onChangePreset(selectedPreset);
                  setUserSelectedPreset(null);
                }}
                disabled={isBusy}
              >
                {busyAction === 'change_preset'
                  ? t('settings.sync.updating_preset', { defaultValue: 'Updating...' })
                  : t('settings.sync.apply_preset', { defaultValue: 'Apply preset change' })}
              </button>
            )}
          </div>
        </SettingsItem>
      </SettingsSection>

      {/* Advanced Security & Key Accordion */}
      <SettingsSection>
        <SettingsAccordion
          defaultOpen={Boolean(recoveryKey)}
          title={
            <div className="settings-accordion-copy">
              <div className="settings-accordion-copy-title">
                <ShieldCheck size={16} />
                {t('settings.sync.security_title', { defaultValue: 'Vault Security & Recovery' })}
              </div>
              <div className="settings-accordion-copy-hint">
                {t('settings.sync.security_hint', {
                  defaultValue: 'Manage your encryption password and emergency recovery keys.',
                })}
              </div>
            </div>
          }
        >
          <div className="sync-security-panel">
            {/* Recovery Key Management */}
            <SettingsItem
              title={t('settings.sync.recovery_key_manage_title', {
                defaultValue: 'Emergency Recovery Key',
              })}
              hint={t('settings.sync.recovery_key_manage_hint', {
                defaultValue: 'Used to recover your data if you forget the master password.',
              })}
            >
              <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => void onGenerateRecoveryKey()}
                  disabled={isBusy}
                >
                  <Sparkles size={14} />
                  <span>
                    {t('settings.sync.regenerate_key_action', { defaultValue: 'Generate new key' })}
                  </span>
                </button>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => void onLock()}
                  disabled={isBusy}
                  title={t('settings.sync.lock_and_test_recovery_action', {
                    defaultValue: 'Lock & test recovery key',
                  })}
                >
                  <Lock size={14} />
                  <span>
                    {t('settings.sync.lock_and_test_recovery_action', {
                      defaultValue: 'Lock & test recovery key',
                    })}
                  </span>
                </button>
              </div>
            </SettingsItem>

            {recoveryKey && (
              <div className="sync-recovery-output-card">
                <div className="sync-recovery-card-header">
                  <ShieldCheck size={16} />
                  <strong>
                    {t('settings.sync.new_recovery_key_title', {
                      defaultValue: 'Active Recovery Key',
                    })}
                  </strong>
                </div>
                <p>
                  {t('settings.sync.recovery_key_save_warning', {
                    defaultValue:
                      'Save this key in a secure location (e.g. password manager). It is not stored in plain text.',
                  })}
                </p>
                <div className="sync-recovery-box">
                  <code className="sync-recovery-key-text">
                    {isKeyRevealed
                      ? formatPartiallyMaskedKey(recoveryKey)
                      : '••••••••••••••••••••••••'}
                  </code>
                  <div className="sync-recovery-actions">
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      onClick={() => setIsKeyRevealed((prev) => !prev)}
                      title={
                        isKeyRevealed
                          ? t('settings.sync.hide_recovery_key', { defaultValue: 'Hide key' })
                          : t('settings.sync.view_recovery_key', { defaultValue: 'View key' })
                      }
                      aria-label={
                        isKeyRevealed
                          ? t('settings.sync.hide_recovery_key', { defaultValue: 'Hide key' })
                          : t('settings.sync.view_recovery_key', { defaultValue: 'View key' })
                      }
                    >
                      {isKeyRevealed ? <EyeOff size={14} /> : <Eye size={14} />}
                      <span>
                        {isKeyRevealed
                          ? t('settings.sync.hide_recovery_key', { defaultValue: 'Hide' })
                          : t('settings.sync.view_recovery_key', { defaultValue: 'View' })}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      onClick={handleCopyKeyWithFeedback}
                      title={t('common.copy', { defaultValue: 'Copy' })}
                    >
                      {copiedKey ? <Check size={14} /> : <Copy size={14} />}
                      <span>
                        {copiedKey
                          ? t('common.copied', { defaultValue: 'Copied' })
                          : t('common.copy', { defaultValue: 'Copy' })}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      onClick={() => void onExportRecoveryKey()}
                      title={t('settings.sync.export_recovery_key', {
                        defaultValue: 'Export recovery key',
                      })}
                    >
                      <Download size={14} />
                      <span>
                        {t('settings.sync.export_recovery_key', { defaultValue: 'Export' })}
                      </span>
                    </button>
                    {onDeleteRecoveryKey && (
                      <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        onClick={() => void handleDeleteRecoveryKey()}
                        title={t('settings.sync.delete_recovery_key', {
                          defaultValue: 'Delete key',
                        })}
                        aria-label={t('settings.sync.delete_recovery_key', {
                          defaultValue: 'Delete key',
                        })}
                      >
                        <Trash2 size={14} />
                        <span>{t('common.delete', { defaultValue: 'Delete' })}</span>
                      </button>
                    )}
                  </div>
                </div>
              </div>
            )}

            {/* Change Password Form */}
            <form
              className="sync-change-password-form"
              onSubmit={(event) => {
                event.preventDefault();
                if (nextPassword !== confirmPassword) {
                  setPasswordError(
                    t('settings.sync.validation_password_match', {
                      defaultValue: 'The new master password confirmation does not match.',
                    })
                  );
                  return;
                }
                setPasswordError(null);
                void onChangeMasterPassword(currentPassword, nextPassword).then(() => {
                  setCurrentPassword('');
                  setNextPassword('');
                  setConfirmPassword('');
                });
              }}
            >
              <div className="sync-security-form-header">
                <KeyRound size={15} />
                <span>
                  {t('settings.sync.change_password', { defaultValue: 'Change master password' })}
                </span>
              </div>

              <div style={{ padding: '0 4px 6px 4px' }}>
                <div className="sync-segmented-control">
                  <button
                    type="button"
                    className={`sync-segmented-btn ${passwordChangeMode === 'password' ? 'is-active' : ''}`}
                    onClick={() => setPasswordChangeMode('password')}
                  >
                    <KeyRound size={13} />
                    <span>
                      {t('settings.sync.change_password_mode_current', {
                        defaultValue: 'Use current password',
                      })}
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`sync-segmented-btn ${passwordChangeMode === 'recovery' ? 'is-active' : ''}`}
                    onClick={() => setPasswordChangeMode('recovery')}
                  >
                    <ShieldCheck size={13} />
                    <span>
                      {t('settings.sync.change_password_mode_recovery', {
                        defaultValue: 'Reset with recovery key',
                      })}
                    </span>
                  </button>
                </div>
              </div>

              <div className="sync-change-password-fields">
                {passwordChangeMode === 'password' ? (
                  <SettingsItem
                    title={t('settings.sync.current_password', {
                      defaultValue: 'Current password',
                    })}
                  >
                    <PasswordInput
                      autoComplete="current-password"
                      value={currentPassword}
                      onChange={(event) => setCurrentPassword(event.target.value)}
                      disabled={isBusy}
                      style={{ width: '100%', maxWidth: '280px' }}
                    />
                  </SettingsItem>
                ) : (
                  <SettingsItem
                    title={t('settings.sync.use_recovery_key', {
                      defaultValue: 'Emergency recovery key',
                    })}
                    hint={t('settings.sync.recovery_key_placeholder', {
                      defaultValue: 'Paste the recovery key saved during vault creation',
                    })}
                  >
                    <input
                      type="text"
                      className="settings-input sync-monospace-input"
                      placeholder={t('settings.sync.recovery_key_placeholder', {
                        defaultValue: 'Paste the recovery key saved during vault creation',
                      })}
                      value={currentPassword}
                      onChange={(event) => setCurrentPassword(event.target.value)}
                      disabled={isBusy}
                      style={{ width: '100%', maxWidth: '380px' }}
                    />
                  </SettingsItem>
                )}

                <SettingsItem
                  title={t('settings.sync.next_password', { defaultValue: 'New master password' })}
                >
                  <PasswordInput
                    autoComplete="new-password"
                    value={nextPassword}
                    onChange={(event) => setNextPassword(event.target.value)}
                    disabled={isBusy}
                    style={{ width: '100%', maxWidth: '280px' }}
                  />
                </SettingsItem>

                <SettingsItem
                  title={t('settings.sync.confirm_next_password', {
                    defaultValue: 'Confirm new master password',
                  })}
                >
                  <PasswordInput
                    autoComplete="new-password"
                    value={confirmPassword}
                    onChange={(event) => setConfirmPassword(event.target.value)}
                    disabled={isBusy}
                    style={{ width: '100%', maxWidth: '280px' }}
                  />
                </SettingsItem>
              </div>

              {passwordError && (
                <div className="sync-banner-box is-error" role="alert" style={{ marginTop: '8px' }}>
                  <AlertCircle size={15} />
                  <span>{passwordError}</span>
                </div>
              )}

              <div className="sync-change-password-actions">
                <button
                  type="submit"
                  className="btn btn-secondary btn-sm"
                  disabled={isBusy || !currentPassword || !nextPassword || !confirmPassword}
                >
                  <KeyRound size={14} />
                  <span>
                    {busyAction === 'change_master_password'
                      ? t('settings.sync.changing_password', {
                          defaultValue: 'Updating password...',
                        })
                      : t('settings.sync.change_password_action', {
                          defaultValue: 'Update password',
                        })}
                  </span>
                </button>
              </div>
            </form>
          </div>
        </SettingsAccordion>
      </SettingsSection>

      {/* Conflict Center */}
      {conflictCenterSlot && <SettingsSection>{conflictCenterSlot}</SettingsSection>}

      {/* Disconnect Danger Zone */}
      <SettingsSection
        title={t('settings.sync.danger_zone_title', { defaultValue: 'Danger Zone' })}
      >
        <SettingsItem
          title={t('settings.sync.disconnect_title', { defaultValue: 'Disconnect this device' })}
          hint={t('settings.sync.disconnect_hint', {
            defaultValue: 'Local data stays on this device. The remote vault is not deleted.',
          })}
        >
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => void onDisconnect()}
            disabled={isBusy}
            style={{ color: 'var(--color-danger, #ef4444)' }}
          >
            <Unplug size={15} />
            <span>{t('settings.sync.disconnect', { defaultValue: 'Disconnect' })}</span>
          </button>
        </SettingsItem>
      </SettingsSection>
      {/* Pairing Modal */}
      <Modal
        isOpen={showPairingModal}
        onClose={() => setShowPairingModal(false)}
        title={t('settings.sync.pair_device_modal_title', { defaultValue: 'Pair New Device' })}
        size="md"
        footer={
          <>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setShowPairingModal(false)}
            >
              {t('common.close', { defaultValue: 'Close' })}
            </button>
            <button type="button" className="btn btn-primary" onClick={handleCopyPairingToken}>
              {copiedToken ? <Check size={14} /> : <Copy size={14} />}
              <span>
                {copiedToken
                  ? t('common.copied', { defaultValue: 'Copied' })
                  : t('settings.sync.copy_pairing_token', { defaultValue: 'Copy pairing code' })}
              </span>
            </button>
          </>
        }
      >
        <div className="sync-pairing-modal-body">
          <p className="sync-pairing-modal-desc">
            {t('settings.sync.pairing_modal_desc', {
              defaultValue:
                'Use this pairing code to quickly connect your second device without re-entering server parameters.',
            })}
          </p>

          <div className="sync-pairing-summary-card">
            <div className="sync-pairing-summary-row">
              <span>{t('settings.sync.server_url', { defaultValue: 'Server URL' })}</span>
              <code title={pairingInfo?.serverUrl || undefined}>
                {pairingInfo?.serverUrl || '—'}
              </code>
            </div>
            <div className="sync-pairing-summary-row">
              <span>{t('settings.sync.username', { defaultValue: 'Username' })}</span>
              <strong>{pairingInfo?.username || '—'}</strong>
            </div>
            <div className="sync-pairing-summary-row">
              <span>{t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}</span>
              <code>{status.vaultId}</code>
            </div>
          </div>

          <div className="sync-pairing-token-container">
            <div className="sync-pairing-token-header">
              <span className="sync-pairing-token-label">
                <Link2 size={13} />
                {t('settings.sync.pairing_token_label', { defaultValue: 'Pairing code' })}
              </span>
              <button
                type="button"
                className="btn btn-secondary btn-sm sync-token-copy-inline-btn"
                onClick={handleCopyPairingToken}
              >
                {copiedToken ? (
                  <Check size={13} style={{ color: 'var(--color-success, #228b4e)' }} />
                ) : (
                  <Copy size={13} />
                )}
                <span>
                  {copiedToken
                    ? t('common.copied', { defaultValue: 'Copied' })
                    : t('common.copy', { defaultValue: 'Copy' })}
                </span>
              </button>
            </div>
            <textarea
              readOnly
              className="sync-pairing-token-textarea"
              value={pairingToken}
              onClick={(e) => (e.target as HTMLTextAreaElement).select()}
              rows={3}
              spellCheck={false}
              aria-label={t('settings.sync.pairing_token_label', { defaultValue: 'Pairing code' })}
            />
          </div>

          <div className="sync-pairing-steps-guide">
            <div className="sync-pairing-step-item">
              <span className="sync-step-badge">1</span>
              <span>
                {t('settings.sync.pair_step_1', {
                  defaultValue: 'Open Sona on your second device',
                })}
              </span>
            </div>
            <div className="sync-pairing-step-item">
              <span className="sync-step-badge">2</span>
              <span>
                {t('settings.sync.pair_step_2', {
                  defaultValue: 'Go to Settings -> Cloud Sync -> click "Import pairing code"',
                })}
              </span>
            </div>
            <div className="sync-pairing-step-item">
              <span className="sync-step-badge">3</span>
              <span>
                {t('settings.sync.pair_step_3', {
                  defaultValue: 'Paste the code above and enter your master password',
                })}
              </span>
            </div>
          </div>

          <div className="sync-banner-box is-security" style={{ marginTop: '2px' }}>
            <ShieldCheck size={15} />
            <span>
              {t('settings.sync.pairing_security_note', {
                defaultValue:
                  'The pairing code contains server metadata only. You will still need to enter your Master Password on the second device to unlock.',
              })}
            </span>
          </div>
        </div>
      </Modal>
    </div>
  );
}

export default SyncConnectedPanel;
