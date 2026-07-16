import React from 'react';
import { DatabaseZap, Link2, Server } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../../Dropdown';
import type {
  SyncCreateRequest,
  SyncCreateResult,
  SyncJoinPreview,
  SyncPreviewJoinRequest,
  SyncProviderDescriptor,
  SyncRunResult,
  WebDavObjectStoreConfig,
} from '../../../types/sync';

type SetupMode = 'create' | 'join';

interface SyncSetupPanelProps {
  busyAction: string | null;
  onCreate: (request: SyncCreateRequest) => Promise<SyncCreateResult>;
  onJoin: (request: SyncPreviewJoinRequest) => Promise<SyncRunResult>;
  onPreviewJoin: (request: SyncPreviewJoinRequest) => Promise<SyncJoinPreview>;
  onTestProvider: (config: WebDavObjectStoreConfig) => Promise<SyncProviderDescriptor>;
}

const EMPTY_PROVIDER: WebDavObjectStoreConfig = {
  serverUrl: '',
  remoteRoot: '',
  username: '',
  password: '',
};

function providerError(config: WebDavObjectStoreConfig): string | null {
  try {
    const url = new URL(config.serverUrl.trim());
    if (url.protocol !== 'https:') {
      return 'https';
    }
  } catch {
    return 'url';
  }
  if (!config.username.trim() || !config.password) {
    return 'credentials';
  }
  return null;
}

export function SyncSetupPanel({
  busyAction,
  onCreate,
  onJoin,
  onPreviewJoin,
  onTestProvider,
}: SyncSetupPanelProps): React.JSX.Element {
  const { t } = useTranslation();
  const [mode, setMode] = React.useState<SetupMode>('create');
  const [provider, setProvider] = React.useState(EMPTY_PROVIDER);
  const [vaultId, setVaultId] = React.useState('');
  const [masterPassword, setMasterPassword] = React.useState('');
  const [confirmPassword, setConfirmPassword] = React.useState('');
  const [preset, setPreset] = React.useState<SyncCreateRequest['preset']>('standard');
  const [createRecoveryKey, setCreateRecoveryKey] = React.useState(true);
  const [preview, setPreview] = React.useState<SyncJoinPreview | null>(null);
  const [validationError, setValidationError] = React.useState<string | null>(null);
  const isBusy = busyAction !== null;

  const updateProvider = (patch: Partial<WebDavObjectStoreConfig>) => {
    setProvider((current) => ({ ...current, ...patch }));
    setPreview(null);
    setValidationError(null);
  };

  const validate = (): boolean => {
    const providerIssue = providerError(provider);
    if (providerIssue === 'https') {
      setValidationError(t('settings.sync.validation_https', {
        defaultValue: 'WebDAV requires an HTTPS server URL.',
      }));
      return false;
    }
    if (providerIssue) {
      setValidationError(t('settings.sync.validation_provider', {
        defaultValue: 'Complete the WebDAV server, username, and password fields.',
      }));
      return false;
    }
    if (!masterPassword) {
      setValidationError(t('settings.sync.validation_master_password', {
        defaultValue: 'Enter a master password.',
      }));
      return false;
    }
    if (mode === 'create' && masterPassword !== confirmPassword) {
      setValidationError(t('settings.sync.validation_password_match', {
        defaultValue: 'The master password confirmation does not match.',
      }));
      return false;
    }
    if (mode === 'join' && !vaultId.trim()) {
      setValidationError(t('settings.sync.validation_vault_id', {
        defaultValue: 'Enter the vault ID from an existing device.',
      }));
      return false;
    }
    setValidationError(null);
    return true;
  };

  const joinRequest = (): SyncPreviewJoinRequest => ({
    provider,
    vaultId: vaultId.trim(),
    masterPassword,
  });

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!validate()) {
      return;
    }
    try {
      if (mode === 'create') {
        await onCreate({
          provider,
          preset,
          masterPassword,
          createRecoveryKey,
        });
        return;
      }
      setPreview(await onPreviewJoin(joinRequest()));
    } catch {
      // The parent reports structured command errors.
    }
  };

  return (
    <div className="sync-setup-panel">
      <div className="sync-segmented-control" role="tablist" aria-label={t('settings.sync.setup_mode', { defaultValue: 'Sync setup mode' })}>
        <button
          type="button"
          role="tab"
          aria-selected={mode === 'create'}
          className={mode === 'create' ? 'active' : ''}
          onClick={() => {
            setMode('create');
            setPreview(null);
            setValidationError(null);
          }}
        >
          <DatabaseZap size={16} />
          {t('settings.sync.create_tab', { defaultValue: 'Create vault' })}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={mode === 'join'}
          className={mode === 'join' ? 'active' : ''}
          onClick={() => {
            setMode('join');
            setPreview(null);
            setValidationError(null);
          }}
        >
          <Link2 size={16} />
          {t('settings.sync.join_tab', { defaultValue: 'Join vault' })}
        </button>
      </div>

      <form className="sync-form" onSubmit={handleSubmit}>
        <div className="sync-form-heading">
          <Server size={18} />
          <span>{t('settings.sync.provider_webdav', { defaultValue: 'WebDAV provider' })}</span>
        </div>
        <div className="sync-form-grid">
          <label className="sync-field sync-field-wide" htmlFor="sync-server-url">
            <span>{t('settings.sync.server_url', { defaultValue: 'Server URL' })}</span>
            <input
              id="sync-server-url"
              className="settings-input"
              type="url"
              inputMode="url"
              autoComplete="url"
              value={provider.serverUrl}
              placeholder="https://dav.example.com/remote.php/dav/files/user"
              onChange={(event) => updateProvider({ serverUrl: event.target.value })}
              disabled={isBusy}
            />
          </label>
          <label className="sync-field sync-field-wide" htmlFor="sync-remote-root">
            <span>{t('settings.sync.remote_root', { defaultValue: 'Remote root' })}</span>
            <input
              id="sync-remote-root"
              className="settings-input"
              type="text"
              value={provider.remoteRoot}
              placeholder="Sona"
              onChange={(event) => updateProvider({ remoteRoot: event.target.value })}
              disabled={isBusy}
            />
          </label>
          <label className="sync-field" htmlFor="sync-provider-username">
            <span>{t('settings.sync.username', { defaultValue: 'Username' })}</span>
            <input
              id="sync-provider-username"
              className="settings-input"
              type="text"
              autoComplete="username"
              value={provider.username}
              onChange={(event) => updateProvider({ username: event.target.value })}
              disabled={isBusy}
            />
          </label>
          <label className="sync-field" htmlFor="sync-provider-password">
            <span>{t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}</span>
            <input
              id="sync-provider-password"
              className="settings-input"
              type="password"
              autoComplete="current-password"
              value={provider.password}
              onChange={(event) => updateProvider({ password: event.target.value })}
              disabled={isBusy}
            />
          </label>
        </div>

        <button
          type="button"
          className="btn btn-secondary sync-test-provider"
          disabled={isBusy || providerError(provider) !== null}
          onClick={() => void onTestProvider(provider).catch(() => undefined)}
        >
          {busyAction === 'test_provider'
            ? t('settings.sync.testing', { defaultValue: 'Testing...' })
            : t('settings.sync.test_provider', { defaultValue: 'Test provider' })}
        </button>

        {mode === 'join' ? (
          <label className="sync-field" htmlFor="sync-vault-id">
            <span>{t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}</span>
            <input
              id="sync-vault-id"
              className="settings-input"
              type="text"
              spellCheck={false}
              value={vaultId}
              onChange={(event) => {
                setVaultId(event.target.value);
                setPreview(null);
              }}
              disabled={isBusy}
            />
          </label>
        ) : (
          <label className="sync-field" htmlFor="sync-preset">
            <span>{t('settings.sync.preset', { defaultValue: 'Sync preset' })}</span>
            <Dropdown
              id="sync-preset"
              value={preset}
              onChange={(value) => setPreset(value as SyncCreateRequest['preset'])}
              disabled={isBusy}
              options={[
                { value: 'content', label: t('settings.sync.preset_content', { defaultValue: 'Content' }) },
                { value: 'standard', label: t('settings.sync.preset_standard', { defaultValue: 'Standard' }) },
                { value: 'full', label: t('settings.sync.preset_full', { defaultValue: 'Full' }) },
              ]}
            />
          </label>
        )}

        <div className="sync-form-grid">
          <label className="sync-field" htmlFor="sync-master-password">
            <span>{t('settings.sync.master_password', { defaultValue: 'Master password' })}</span>
            <input
              id="sync-master-password"
              className="settings-input"
              type="password"
              autoComplete={mode === 'create' ? 'new-password' : 'current-password'}
              value={masterPassword}
              onChange={(event) => {
                setMasterPassword(event.target.value);
                setPreview(null);
              }}
              disabled={isBusy}
            />
          </label>
          {mode === 'create' ? (
            <label className="sync-field" htmlFor="sync-master-password-confirm">
              <span>{t('settings.sync.confirm_master_password', { defaultValue: 'Confirm master password' })}</span>
              <input
                id="sync-master-password-confirm"
                className="settings-input"
                type="password"
                autoComplete="new-password"
                value={confirmPassword}
                onChange={(event) => setConfirmPassword(event.target.value)}
                disabled={isBusy}
              />
            </label>
          ) : null}
        </div>

        {mode === 'create' ? (
          <label className="sync-checkbox-row">
            <input
              type="checkbox"
              checked={createRecoveryKey}
              onChange={(event) => setCreateRecoveryKey(event.target.checked)}
              disabled={isBusy}
            />
            <span>{t('settings.sync.create_recovery_key', { defaultValue: 'Create a recovery key' })}</span>
          </label>
        ) : null}

        {validationError ? <div className="sync-inline-error" role="alert">{validationError}</div> : null}

        {preview ? (
          <div className="sync-join-preview" role="status">
            <div>
              <span>{t('settings.sync.preview_local', { defaultValue: 'Local operations' })}</span>
              <strong>{preview.localOperationCount}</strong>
            </div>
            <div>
              <span>{t('settings.sync.preview_remote', { defaultValue: 'Remote operations' })}</span>
              <strong>{preview.remoteOperationCount}</strong>
            </div>
            <div>
              <span>{t('settings.sync.preview_conflicts', { defaultValue: 'Projected conflicts' })}</span>
              <strong>{preview.projectedConflictCount}</strong>
            </div>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void onJoin(joinRequest()).catch(() => undefined)}
              disabled={isBusy}
            >
              {busyAction === 'join'
                ? t('settings.sync.joining', { defaultValue: 'Joining...' })
                : t('settings.sync.confirm_join', { defaultValue: 'Confirm join' })}
            </button>
          </div>
        ) : (
          <button type="submit" className="btn btn-primary" disabled={isBusy}>
            {mode === 'create'
              ? (busyAction === 'create'
                  ? t('settings.sync.creating', { defaultValue: 'Creating...' })
                  : t('settings.sync.create_action', { defaultValue: 'Create sync vault' }))
              : (busyAction === 'preview_join'
                  ? t('settings.sync.previewing', { defaultValue: 'Preparing preview...' })
                  : t('settings.sync.preview_join', { defaultValue: 'Preview join' }))}
          </button>
        )}
      </form>
    </div>
  );
}
