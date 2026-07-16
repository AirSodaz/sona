import React from 'react';
import {
  Cloud,
  Copy,
  Download,
  KeyRound,
  Lock,
  Pause,
  Play,
  RefreshCw,
  Unplug,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../../Dropdown';
import type {
  SyncPresetV1,
  SyncStatusSnapshot,
  SyncUnlockRecoveryRequest,
  SyncUnlockRequest,
} from '../../../types/sync';
import { SettingsAccordion, SettingsItem } from '../SettingsLayout';

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
  const isBusy = busyAction !== null || status.state === 'syncing';

  const stateLabel = t(`settings.sync.status_${status.state}`, {
    defaultValue: status.state,
  });
  const retryLabel = formatDate(
    status.nextRetryAtMs,
    t('settings.sync.not_scheduled', { defaultValue: 'Not scheduled' }),
  );

  if (status.state === 'locked') {
    const canUnlock = providerPassword.length > 0
      && (unlockMode === 'password' ? masterPassword.length > 0 : recoveryInput.length > 0);
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
          <div className="sync-status-icon"><Lock size={18} /></div>
          <div>
            <strong>{t('settings.sync.locked_title', { defaultValue: 'Sync vault locked' })}</strong>
            <span>{status.vaultId}</span>
          </div>
        </div>
        <div className="sync-segmented-control" role="tablist" aria-label={t('settings.sync.unlock_mode', { defaultValue: 'Unlock method' })}>
          <button type="button" role="tab" aria-selected={unlockMode === 'password'} className={unlockMode === 'password' ? 'active' : ''} onClick={() => setUnlockMode('password')}>
            {t('settings.sync.master_password', { defaultValue: 'Master password' })}
          </button>
          <button type="button" role="tab" aria-selected={unlockMode === 'recovery'} className={unlockMode === 'recovery' ? 'active' : ''} onClick={() => setUnlockMode('recovery')}>
            {t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}
          </button>
        </div>
        <label className="sync-field" htmlFor="sync-unlock-provider-password">
          <span>{t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}</span>
          <input
            id="sync-unlock-provider-password"
            className="settings-input"
            type="password"
            autoComplete="current-password"
            value={providerPassword}
            onChange={(event) => setProviderPassword(event.target.value)}
            disabled={isBusy}
          />
        </label>
        {unlockMode === 'password' ? (
          <label className="sync-field" htmlFor="sync-unlock-master-password">
            <span>{t('settings.sync.master_password', { defaultValue: 'Master password' })}</span>
            <input
              id="sync-unlock-master-password"
              className="settings-input"
              type="password"
            autoComplete="current-password"
              value={masterPassword}
              onChange={(event) => setMasterPassword(event.target.value)}
              disabled={isBusy}
            />
          </label>
        ) : (
          <label className="sync-field" htmlFor="sync-unlock-recovery-key">
            <span>{t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}</span>
            <input
              id="sync-unlock-recovery-key"
              className="settings-input sync-monospace-input"
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={recoveryInput}
              onChange={(event) => setRecoveryInput(event.target.value)}
              disabled={isBusy}
            />
          </label>
        )}
        <button type="submit" className="btn btn-primary" disabled={isBusy || !canUnlock}>
          <KeyRound size={16} />
          {busyAction === 'unlock'
            ? t('settings.sync.unlocking', { defaultValue: 'Unlocking...' })
            : t('settings.sync.unlock_action', { defaultValue: 'Unlock' })}
        </button>
      </form>
    );
  }

  return (
    <div className="sync-connected-panel">
      <div className={`sync-status-panel is-${status.state}`}>
        <div className="sync-status-heading">
          <div className="sync-status-icon"><Cloud size={18} /></div>
          <div>
            <strong>{stateLabel}</strong>
            <span>{status.providerId === 'webdav' ? 'WebDAV' : status.providerId}</span>
          </div>
        </div>
        <div className="sync-status-grid">
          <div><span>{t('settings.sync.preset', { defaultValue: 'Sync preset' })}</span><strong>{t(`settings.sync.preset_${status.preset}`, { defaultValue: status.preset ?? '-' })}</strong></div>
          <div><span>{t('settings.sync.last_success', { defaultValue: 'Last success' })}</span><strong>{formatDate(status.lastSuccessAtMs, t('settings.sync.never', { defaultValue: 'Never' }))}</strong></div>
          <div><span>{t('settings.sync.pending', { defaultValue: 'Pending upload' })}</span><strong>{status.pendingOperationCount}</strong></div>
          <div><span>{t('settings.sync.conflicts', { defaultValue: 'Conflicts' })}</span><strong>{status.conflictCount}</strong></div>
          {status.state === 'error' ? (
            <div className="sync-status-wide"><span>{t('settings.sync.next_retry', { defaultValue: 'Next retry' })}</span><strong>{retryLabel}</strong></div>
          ) : null}
        </div>
        {status.lastError ? (
          <div className="sync-error-banner" role="alert">
            <strong>{status.lastError.code}</strong>
            <span>{status.lastError.message}</span>
          </div>
        ) : null}
        <div className="sync-actions">
          <button type="button" className="btn btn-primary" onClick={() => void onRunNow()} disabled={isBusy || status.state === 'paused'}>
            <RefreshCw size={16} className={status.state === 'syncing' ? 'queue-icon-spin' : undefined} />
            {status.state === 'syncing'
              ? t('settings.sync.syncing', { defaultValue: 'Syncing...' })
              : t('settings.sync.run_now', { defaultValue: 'Sync now' })}
          </button>
          <button type="button" className="btn btn-secondary" onClick={() => void onSetPaused(status.state !== 'paused')} disabled={isBusy}>
            {status.state === 'paused' ? <Play size={16} /> : <Pause size={16} />}
            {status.state === 'paused'
              ? t('settings.sync.resume', { defaultValue: 'Resume' })
              : t('settings.sync.pause', { defaultValue: 'Pause' })}
          </button>
          <button type="button" className="btn btn-secondary" onClick={() => void onLock()} disabled={isBusy}>
            <Lock size={16} />
            {t('settings.sync.lock_action', { defaultValue: 'Lock' })}
          </button>
        </div>
      </div>

      <SettingsItem
        title={t('settings.sync.preset', { defaultValue: 'Sync preset' })}
        hint={t('settings.sync.preset_hint', { defaultValue: 'Choose which portable data domains participate in sync.' })}
      >
        <div className="sync-inline-control">
          <Dropdown
            value={selectedPreset}
            onChange={(value) => setSelectedPreset(value as SyncPresetV1)}
            disabled={isBusy}
            options={[
              { value: 'content', label: t('settings.sync.preset_content', { defaultValue: 'Content' }) },
              { value: 'standard', label: t('settings.sync.preset_standard', { defaultValue: 'Standard' }) },
              { value: 'full', label: t('settings.sync.preset_full', { defaultValue: 'Full' }) },
            ]}
          />
          <button type="button" className="btn btn-secondary" disabled={isBusy || selectedPreset === status.preset} onClick={() => void onChangePreset(selectedPreset)}>
            {t('common.apply', { defaultValue: 'Apply' })}
          </button>
        </div>
      </SettingsItem>

      <SettingsAccordion
        title={t('settings.sync.security', { defaultValue: 'Security' })}
        defaultOpen={Boolean(recoveryKey)}
      >
        <div className="sync-security-panel">
          <form
            className="sync-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (!nextPassword) {
                setPasswordError(t('settings.sync.validation_master_password', { defaultValue: 'Enter a master password.' }));
                return;
              }
              if (nextPassword !== confirmPassword) {
                setPasswordError(t('settings.sync.validation_password_match', { defaultValue: 'The master password confirmation does not match.' }));
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
            <div className="sync-form-heading"><KeyRound size={17} /><span>{t('settings.sync.change_password', { defaultValue: 'Change master password' })}</span></div>
            <div className="sync-form-grid">
              <label className="sync-field"><span>{t('settings.sync.current_password', { defaultValue: 'Current password' })}</span><input className="settings-input" type="password" autoComplete="current-password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} disabled={isBusy} /></label>
              <label className="sync-field"><span>{t('settings.sync.next_password', { defaultValue: 'New password' })}</span><input className="settings-input" type="password" autoComplete="new-password" value={nextPassword} onChange={(event) => setNextPassword(event.target.value)} disabled={isBusy} /></label>
              <label className="sync-field"><span>{t('settings.sync.confirm_master_password', { defaultValue: 'Confirm master password' })}</span><input className="settings-input" type="password" autoComplete="new-password" value={confirmPassword} onChange={(event) => setConfirmPassword(event.target.value)} disabled={isBusy} /></label>
            </div>
            {passwordError ? <div className="sync-inline-error" role="alert">{passwordError}</div> : null}
            <button type="submit" className="btn btn-secondary" disabled={isBusy || !currentPassword || !nextPassword || !confirmPassword}>{t('settings.sync.change_password_action', { defaultValue: 'Change password' })}</button>
          </form>

          <div className="sync-recovery-control">
            <div><strong>{t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}</strong><span>{t('settings.sync.recovery_key_hint', { defaultValue: 'Generating a key replaces the previous recovery key.' })}</span></div>
            <button type="button" className="btn btn-secondary" onClick={() => void onGenerateRecoveryKey()} disabled={isBusy}><KeyRound size={16} />{t('settings.sync.generate_recovery_key', { defaultValue: 'Generate key' })}</button>
          </div>
          {recoveryKey ? (
            <div className="sync-recovery-output">
              <code>{recoveryKey}</code>
              <button type="button" className="btn btn-icon" onClick={() => void onCopyRecoveryKey()} aria-label={t('common.copy', { defaultValue: 'Copy' })} data-tooltip={t('common.copy', { defaultValue: 'Copy' })}><Copy size={16} /></button>
              <button type="button" className="btn btn-icon" onClick={() => void onExportRecoveryKey()} aria-label={t('settings.sync.export_recovery_key', { defaultValue: 'Export recovery key' })} data-tooltip={t('settings.sync.export_recovery_key', { defaultValue: 'Export recovery key' })}><Download size={16} /></button>
            </div>
          ) : null}
        </div>
      </SettingsAccordion>

      <SettingsItem
        title={t('settings.sync.disconnect_title', { defaultValue: 'Disconnect this device' })}
        hint={t('settings.sync.disconnect_hint', { defaultValue: 'Local data stays on this device. The remote vault is not deleted.' })}
      >
        <button type="button" className="btn btn-secondary" onClick={() => void onDisconnect()} disabled={isBusy}>
          <Unplug size={16} />
          {t('settings.sync.disconnect_action', { defaultValue: 'Disconnect' })}
        </button>
      </SettingsItem>
    </div>
  );
}
