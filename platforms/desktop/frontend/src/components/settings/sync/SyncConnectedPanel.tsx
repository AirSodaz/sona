import React from 'react';
import {
  AlertCircle,
  AlertTriangle,
  Check,
  Cloud,
  Copy,
  Download,
  KeyRound,
  Layers,
  Lock,
  Pause,
  Play,
  QrCode,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Unplug,
  X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type {
  SyncPresetV1,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
} from '../../../types/sync';
import { SettingsAccordion, SettingsItem } from '../SettingsLayout';
import { encodeSyncPairingToken } from './syncPairing';

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
}

function formatDate(value: number | null, fallback: string): string {
  return value ? new Date(value).toLocaleString() : fallback;
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
}: SyncConnectedPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const [unlockMode, setUnlockMode] = React.useState<'password' | 'recovery'>('password');
  const [providerPassword, setProviderPassword] = React.useState('');
  const [masterPassword, setMasterPassword] = React.useState('');
  const [recoveryInput, setRecoveryInput] = React.useState('');
  const [selectedPreset, setSelectedPreset] = React.useState<SyncPresetV1>(status.preset ?? 'standard');
  const [currentPassword, setCurrentPassword] = React.useState('');
  const [nextPassword, setNextPassword] = React.useState('');
  const [confirmPassword, setConfirmPassword] = React.useState('');
  const [passwordError, setPasswordError] = React.useState<string | null>(null);

  // Pairing Modal state
  const [showPairingModal, setShowPairingModal] = React.useState(false);
  const [copiedToken, setCopiedToken] = React.useState(false);
  const [copiedKey, setCopiedKey] = React.useState(false);

  const isBusy = busyAction !== null || status.state === 'syncing';

  const stateLabel = t(`settings.sync.status_${status.state}`, {
    defaultValue: status.state,
  });
  const retryLabel = formatDate(
    status.nextRetryAtMs,
    t('settings.sync.not_scheduled', { defaultValue: 'Not scheduled' }),
  );

  // Generate pairing token for the current vault
  const pairingToken = React.useMemo(() => {
    if (!status.vaultId) return '';
    return encodeSyncPairingToken(
      {
        serverUrl: '', // Host platform manages endpoint
        remoteRoot: 'Sona',
        username: '',
        password: '',
      },
      status.vaultId,
      false,
    );
  }, [status.vaultId]);

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
            <strong>{t('settings.sync.locked_title', { defaultValue: 'Sync vault locked' })}</strong>
            <span style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>{status.vaultId}</span>
          </div>
        </div>

        <div
          className="settings-scenario-cards"
          role="tablist"
          aria-label={t('settings.sync.unlock_mode', { defaultValue: 'Unlock method' })}
        >
          <button
            type="button"
            role="tab"
            aria-selected={unlockMode === 'password'}
            className={`settings-scenario-card${unlockMode === 'password' ? ' active' : ''}`}
            onClick={() => setUnlockMode('password')}
          >
            <span className="settings-scenario-card-icon"><KeyRound size={18} /></span>
            <span className="settings-scenario-card-text">
              <span className="settings-scenario-card-label">{t('settings.sync.master_password', { defaultValue: 'Master password' })}</span>
            </span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={unlockMode === 'recovery'}
            className={`settings-scenario-card${unlockMode === 'recovery' ? ' active' : ''}`}
            onClick={() => setUnlockMode('recovery')}
          >
            <span className="settings-scenario-card-icon"><ShieldCheck size={18} /></span>
            <span className="settings-scenario-card-text">
              <span className="settings-scenario-card-label">{t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}</span>
            </span>
          </button>
        </div>

        <SettingsItem
          title={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
          hint={t('settings.sync.provider_password_hint', { defaultValue: 'Enter your WebDAV application password' })}
        >
          <input
            id="sync-unlock-provider-password"
            className="settings-input"
            type="password"
            autoComplete="current-password"
            aria-label={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
            value={providerPassword}
            onChange={(event) => setProviderPassword(event.target.value)}
            disabled={isBusy}
            style={{ width: '260px' }}
          />
        </SettingsItem>

        {unlockMode === 'password' ? (
          <SettingsItem
            title={t('settings.sync.master_password', { defaultValue: 'Master password' })}
            hint={t('settings.sync.master_password_hint', { defaultValue: 'Enter your vault master password' })}
          >
            <input
              id="sync-unlock-master-password"
              className="settings-input"
              type="password"
              autoComplete="current-password"
              aria-label={t('settings.sync.master_password', { defaultValue: 'Master password' })}
              value={masterPassword}
              onChange={(event) => setMasterPassword(event.target.value)}
              disabled={isBusy}
              style={{ width: '260px' }}
            />
          </SettingsItem>
        ) : (
          <SettingsItem
            title={t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}
            hint={t('settings.sync.recovery_key_hint', { defaultValue: 'Enter your emergency recovery key' })}
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

        <div className="sync-wizard-actions">
          <div className="sync-actions-spacer" />
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
      {/* Status Panel Card - Sona Unified LocationCard Style */}
      <div className={`sync-status-panel is-${status.state}`}>
        <div className="sync-status-header-row">
          <div className="sync-status-heading">
            <div className="sync-status-icon">
              <Cloud size={20} className={status.state === 'syncing' ? 'queue-icon-spin' : undefined} />
            </div>
            <div>
              <div className="sync-status-title-group">
                <strong>{stateLabel}</strong>
                <span className="sync-provider-pill">
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
              <span className="sync-vault-id-label">
                Vault: <code>{status.vaultId}</code>
              </span>
            </div>
          </div>

          <div className="sync-header-actions-group">
            <button
              type="button"
              className="btn btn-secondary btn-sm sync-pair-device-btn"
              onClick={() => setShowPairingModal(true)}
              disabled={isBusy}
            >
              <QrCode size={15} />
              <span>{t('settings.sync.pair_device_action', { defaultValue: 'Pair new device' })}</span>
            </button>
          </div>
        </div>

        {/* Status Metrics Grid */}
        <div className="sync-status-grid">
          <div>
            <span>{t('settings.sync.last_success', { defaultValue: 'Last success' })}</span>
            <strong>{formatDate(status.lastSuccessAtMs, t('settings.sync.never', { defaultValue: 'Never' }))}</strong>
          </div>
          <div>
            <span>{t('settings.sync.pending', { defaultValue: 'Pending upload' })}</span>
            <strong>{status.pendingOperationCount}</strong>
          </div>
          <div>
            <span>{t('settings.sync.preset', { defaultValue: 'Sync preset' })}</span>
            <strong>{t(`settings.sync.preset_${status.preset}`, { defaultValue: status.preset ?? '-' })}</strong>
          </div>
          <div>
            <span>{t('settings.sync.conflicts', { defaultValue: 'Conflicts' })}</span>
            <strong className={status.conflictCount > 0 ? 'sync-text-warning' : ''}>
              {status.conflictCount}
            </strong>
          </div>
          {status.state === 'error' && (
            <div className="sync-status-wide">
              <span>{t('settings.sync.next_retry', { defaultValue: 'Next retry' })}</span>
              <strong>{retryLabel}</strong>
            </div>
          )}
        </div>

        {/* Error Banner */}
        {status.lastError && (
          <div className="sync-error-banner" role="alert">
            <div className="sync-error-banner-title">
              <AlertCircle size={16} />
              <strong>{status.lastError.code}</strong>
            </div>
            <span>{status.lastError.message}</span>
          </div>
        )}

        {/* Quick Action Buttons */}
        <div className="sync-actions">
          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={() => void onRunNow()}
            disabled={isBusy || status.state === 'paused'}
          >
            <RefreshCw size={15} className={status.state === 'syncing' ? 'queue-icon-spin' : undefined} />
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
            {status.state === 'paused' ? <Play size={15} /> : <Pause size={15} />}
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
          >
            <Lock size={15} />
            <span>{t('settings.sync.lock_action', { defaultValue: 'Lock vault' })}</span>
          </button>
        </div>
      </div>

      {/* Sync Scope Selection */}
      <SettingsItem
        title={t('settings.sync.preset', { defaultValue: 'Sync preset' })}
        hint={t('settings.sync.preset_hint', {
          defaultValue: 'Choose which data domains participate in cloud sync.',
        })}
        layout="vertical"
      >
        <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="settings-scenario-cards three-columns" style={{ width: '100%', padding: 0, background: 'transparent' }}>
            {[
              {
                id: 'content' as const,
                label: t('settings.sync.preset_content', { defaultValue: 'Content' }),
                description: t('settings.sync.scope_transcripts_only', { defaultValue: 'Transcripts & Projects' }),
              },
              {
                id: 'standard' as const,
                label: t('settings.sync.preset_standard', { defaultValue: 'Standard' }),
                description: t('settings.sync.scope_standard_desc', { defaultValue: 'Transcripts, summaries, rules' }),
                badge: t('common.recommended', { defaultValue: 'Recommended' }),
              },
              {
                id: 'full' as const,
                label: t('settings.sync.preset_full', { defaultValue: 'Full' }),
                description: t('settings.sync.scope_full_desc', { defaultValue: 'Full workspace data' }),
              },
            ].map((p) => {
              const isSelected = selectedPreset === p.id;
              return (
                <button
                  key={p.id}
                  type="button"
                  className={`settings-scenario-card${isSelected ? ' active' : ''}`}
                  onClick={() => setSelectedPreset(p.id)}
                  disabled={isBusy}
                >
                  <span className="settings-scenario-card-icon">
                    <Layers size={18} />
                  </span>
                  <span className="settings-scenario-card-text">
                    <span className="settings-scenario-card-label">
                      {p.label}
                      {p.badge && <span className="sync-scope-tag is-badge" style={{ marginLeft: '6px' }}>{p.badge}</span>}
                    </span>
                    <span className="settings-scenario-card-description">
                      {p.description}
                    </span>
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
              onClick={() => void onChangePreset(selectedPreset)}
              disabled={isBusy}
            >
              {busyAction === 'change_preset'
                ? t('settings.sync.updating_preset', { defaultValue: 'Updating...' })
                : t('settings.sync.apply_preset', { defaultValue: 'Apply preset change' })}
            </button>
          )}
        </div>
      </SettingsItem>

      {/* Advanced Security & Key Accordion */}
      <SettingsAccordion
        title={(
          <div className="settings-accordion-copy">
            <div className="settings-accordion-copy-title"><ShieldCheck size={16} />{t('settings.sync.security_title', { defaultValue: 'Vault Security & Recovery' })}</div>
            <div className="settings-accordion-copy-hint">{t('settings.sync.security_hint', { defaultValue: 'Manage your encryption password and emergency recovery keys.' })}</div>
          </div>
        )}
      >
        <div className="sync-security-panel">
          {/* Recovery Key Management */}
          <div className="sync-recovery-control">
            <div>
              <strong>{t('settings.sync.recovery_key_manage_title', { defaultValue: 'Emergency Recovery Key' })}</strong>
              <span>{t('settings.sync.recovery_key_manage_hint', { defaultValue: 'Used to recover your data if you forget the master password.' })}</span>
            </div>
            <div style={{ display: 'flex', gap: '8px' }}>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => void onGenerateRecoveryKey()}
                disabled={isBusy}
              >
                <Sparkles size={15} />
                <span>{t('settings.sync.regenerate_key_action', { defaultValue: 'Generate new key' })}</span>
              </button>
            </div>
          </div>

          {recoveryKey && (
            <div className="sync-recovery-output-card">
              <div className="sync-recovery-card-header">
                <ShieldCheck size={16} />
                <strong>{t('settings.sync.new_recovery_key_title', { defaultValue: 'Active Recovery Key' })}</strong>
              </div>
              <p>{t('settings.sync.recovery_key_save_warning', { defaultValue: 'Save this key in a secure location (e.g. password manager). It is not stored in plain text.' })}</p>
              <div className="sync-recovery-box">
                <code className="sync-recovery-key-text">{recoveryKey}</code>
                <div className="sync-recovery-actions">
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleCopyKeyWithFeedback}
                  >
                    {copiedKey ? <Check size={14} /> : <Copy size={14} />}
                    <span>{copiedKey ? t('common.copied', { defaultValue: 'Copied' }) : t('common.copy', { defaultValue: 'Copy' })}</span>
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => void onExportRecoveryKey()}
                  >
                    <Download size={14} />
                    <span>{t('settings.sync.export_recovery_key', { defaultValue: 'Export recovery key' })}</span>
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Change Password Form */}
          <form
            className="sync-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (nextPassword !== confirmPassword) {
                setPasswordError(t('settings.sync.validation_password_match', { defaultValue: 'The new master password confirmation does not match.' }));
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
            <div className="sync-form-heading">
              <KeyRound size={16} />
              <span>{t('settings.sync.change_password', { defaultValue: 'Change master password' })}</span>
            </div>

            <SettingsItem
              title={t('settings.sync.current_password', { defaultValue: 'Current password' })}
            >
              <input
                className="settings-input"
                type="password"
                autoComplete="current-password"
                value={currentPassword}
                onChange={(event) => setCurrentPassword(event.target.value)}
                disabled={isBusy}
                style={{ width: '260px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.next_password', { defaultValue: 'New master password' })}
            >
              <input
                className="settings-input"
                type="password"
                autoComplete="new-password"
                value={nextPassword}
                onChange={(event) => setNextPassword(event.target.value)}
                disabled={isBusy}
                style={{ width: '260px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.confirm_next_password', { defaultValue: 'Confirm new master password' })}
            >
              <input
                className="settings-input"
                type="password"
                autoComplete="new-password"
                value={confirmPassword}
                onChange={(event) => setConfirmPassword(event.target.value)}
                disabled={isBusy}
                style={{ width: '260px' }}
              />
            </SettingsItem>

            {passwordError && (
              <div className="sync-inline-error" role="alert">
                <span>{passwordError}</span>
              </div>
            )}

            <div style={{ display: 'flex', justifyContent: 'flex-end', paddingTop: '8px' }}>
              <button
                type="submit"
                className="btn btn-secondary btn-sm"
                disabled={isBusy || !currentPassword || !nextPassword || !confirmPassword}
              >
                {busyAction === 'change_master_password'
                  ? t('settings.sync.changing_password', { defaultValue: 'Updating password...' })
                  : t('settings.sync.change_password_action', { defaultValue: 'Update password' })}
              </button>
            </div>
          </form>
        </div>
      </SettingsAccordion>

      {/* Disconnect Danger Zone */}
      <SettingsItem
        title={t('settings.sync.disconnect_title', { defaultValue: 'Disconnect this device' })}
        hint={t('settings.sync.disconnect_hint', { defaultValue: 'Local data stays on this device. The remote vault is not deleted.' })}
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

      {/* Pairing Modal */}
      {showPairingModal && (
        <div className="modal-backdrop" onClick={() => setShowPairingModal(false)}>
          <div className="modal-container sync-pairing-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div className="modal-title-group">
                <QrCode size={18} />
                <h3>{t('settings.sync.pair_device_modal_title', { defaultValue: 'Pair New Device' })}</h3>
              </div>
              <button
                type="button"
                className="modal-close-btn"
                onClick={() => setShowPairingModal(false)}
                aria-label={t('common.close', { defaultValue: 'Close' })}
              >
                <X size={18} />
              </button>
            </div>

            <div className="modal-body sync-pairing-modal-body">
              <p className="sync-pairing-intro">
                {t('settings.sync.pairing_modal_desc', {
                  defaultValue: 'Copy the pairing code below and paste it on your second device under "Join sync vault" -> "Paste pairing code".',
                })}
              </p>

              <div className="sync-pairing-token-box">
                <code>{pairingToken}</code>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={handleCopyPairingToken}
                >
                  {copiedToken ? <Check size={14} /> : <Copy size={14} />}
                  <span>{copiedToken ? t('common.copied', { defaultValue: 'Copied' }) : t('common.copy', { defaultValue: 'Copy code' })}</span>
                </button>
              </div>

              <div className="sync-pairing-security-note">
                <ShieldCheck size={16} />
                <span>
                  {t('settings.sync.pairing_security_note', {
                    defaultValue: 'The pairing code contains encrypted connection parameters only. You will still need to enter your Master Password on the second device to unlock.',
                  })}
                </span>
              </div>
            </div>

            <div className="modal-footer">
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={() => setShowPairingModal(false)}
              >
                {t('common.done', { defaultValue: 'Done' })}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default SyncConnectedPanel;
