import {
  AlertCircle,
  Check,
  CheckCircle2,
  ClipboardPaste,
  DatabaseZap,
  ExternalLink,
  KeyRound,
  Layers,
  Link2,
  RefreshCw,
  Server,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import type {
  AnySyncProviderConfig,
  DiscoveredVaultSummary,
  S3ObjectStoreConfig,
  SyncCreateRequest,
  SyncCreateResult,
  SyncJoinPreview,
  SyncJoinRequest,
  SyncPresetV1,
  SyncPreviewJoinRequest,
  SyncProviderDescriptor,
  SyncRunResult,
  WebDavObjectStoreConfig,
} from '../../../types/sync';
import { Dropdown, type DropdownOption } from '../../Dropdown';
import { Modal } from '../../Modal';
import { Switch } from '../../Switch';
import { SettingsAccordion, SettingsItem, SettingsSection } from '../SettingsLayout';
import { PasswordInput } from './PasswordInput';
import {
  detectProviderPresetId,
  detectS3ProviderPresetId,
  S3_PROVIDER_PRESETS,
  SYNC_PROVIDER_PRESETS,
  type SyncProtocolType,
  type WellKnownS3ProviderId,
  type WellKnownSyncProviderId,
} from './SyncProviderPresets';
import { decodeSyncPairingToken, isS3PairingPayload, isWebDavPairingPayload } from './syncPairing';
import { validateSyncServerUrl } from './syncUrl';
export interface SyncSetupPanelProps {
  busyAction: string | null;
  onCreate: (request: SyncCreateRequest) => Promise<SyncCreateResult>;
  onJoin: (request: SyncJoinRequest) => Promise<SyncRunResult>;
  onPreviewJoin: (request: SyncPreviewJoinRequest) => Promise<SyncJoinPreview>;
  onTestProvider: (config: AnySyncProviderConfig) => Promise<SyncProviderDescriptor>;
  onDiscoverVaults?: (config: AnySyncProviderConfig) => Promise<DiscoveredVaultSummary[]>;
  initialProtocolType?: SyncProtocolType;
}

function checkProviderFields(config: WebDavObjectStoreConfig): string | null {
  const urlValidation = validateSyncServerUrl(config.serverUrl);
  if (!urlValidation.valid) {
    return urlValidation.error ?? 'invalid';
  }
  if (!config.remoteRoot.trim() || !config.username.trim() || !config.password) {
    return 'incomplete';
  }
  return null;
}
function checkS3ProviderFields(config: S3ObjectStoreConfig): string | null {
  const urlValidation = validateSyncServerUrl(config.endpoint);
  if (!urlValidation.valid) {
    return urlValidation.error ?? 'invalid';
  }
  if (
    !config.bucket.trim() ||
    !config.region.trim() ||
    !config.accessKeyId.trim() ||
    !config.secretAccessKey
  ) {
    return 'incomplete';
  }
  return null;
}

export function SyncSetupPanel({
  busyAction,
  onCreate,
  onJoin,
  onTestProvider,
  onDiscoverVaults,
  initialProtocolType,
}: SyncSetupPanelProps): React.JSX.Element {
  const { t } = useTranslation();

  // Form states
  const [protocolType, setProtocolType] = React.useState<SyncProtocolType>(
    initialProtocolType ?? 'webdav'
  );
  const [selectedPresetId, setSelectedPresetId] =
    React.useState<WellKnownSyncProviderId>('nutstore');
  const [provider, setProvider] = React.useState<WebDavObjectStoreConfig>({
    serverUrl: SYNC_PROVIDER_PRESETS[0].defaultServerUrl,
    remoteRoot: 'Sona',
    username: '',
    password: '',
  });
  const [selectedS3PresetId, setSelectedS3PresetId] =
    React.useState<WellKnownS3ProviderId>('cloudflare-r2');
  const [s3Provider, setS3Provider] = React.useState<S3ObjectStoreConfig>({
    endpoint: S3_PROVIDER_PRESETS[0].defaultEndpoint,
    region: S3_PROVIDER_PRESETS[0].defaultRegion,
    bucket: S3_PROVIDER_PRESETS[0].defaultBucket,
    remoteRoot: S3_PROVIDER_PRESETS[0].defaultRemoteRoot,
    accessKeyId: '',
    secretAccessKey: '',
    forcePathStyle: S3_PROVIDER_PRESETS[0].defaultForcePathStyle,
  });

  const activeProviderConfig: AnySyncProviderConfig = protocolType === 's3' ? s3Provider : provider;
  const [masterPassword, setMasterPassword] = React.useState('');
  const [confirmPassword, setConfirmPassword] = React.useState('');
  const [preset, setPreset] = React.useState<SyncPresetV1>('standard');
  const [vaultId, setVaultId] = React.useState('');
  const [createRecoveryKey, setCreateRecoveryKey] = React.useState(true);
  const [unlockMethod, setUnlockMethod] = React.useState<'password' | 'recovery'>('password');
  // Modals & Notices
  const [showPairingModal, setShowPairingModal] = React.useState(false);
  const [pairingTokenInput, setPairingTokenInput] = React.useState('');
  const [pairingSuccessNotice, setPairingSuccessNotice] = React.useState<string | null>(null);
  const [pairingTokenError, setPairingTokenError] = React.useState<string | null>(null);

  // Multi-vault resolution modal state
  const [discoveredVaults, setDiscoveredVaults] = React.useState<DiscoveredVaultSummary[] | null>(
    null
  );
  const [selectedVaultToJoin, setSelectedVaultToJoin] = React.useState<string>('default');
  const [isCreatingNewVault, setIsCreatingNewVault] = React.useState(false);

  // Action states
  const [validationError, setValidationError] = React.useState<string | null>(null);
  const [testSuccess, setTestSuccess] = React.useState<string | null>(null);
  const [testError, setTestError] = React.useState<string | null>(null);
  const [isTesting, setIsTesting] = React.useState(false);
  const [isConnecting, setIsConnecting] = React.useState(false);

  const isBusy = Boolean(busyAction) || isTesting || isConnecting;

  const updateProvider = (patch: Partial<WebDavObjectStoreConfig>) => {
    setProvider((prev) => {
      const next = { ...prev, ...patch };
      if (patch.serverUrl !== undefined) {
        const detected = detectProviderPresetId(patch.serverUrl);
        if (detected !== selectedPresetId) {
          setSelectedPresetId(detected);
        }
      }
      return next;
    });
    setTestSuccess(null);
    setTestError(null);
    setValidationError(null);
  };

  const handlePresetChange = (id: string) => {
    const presetId = id as WellKnownSyncProviderId;
    setSelectedPresetId(presetId);
    const meta = SYNC_PROVIDER_PRESETS.find((p) => p.id === presetId);
    if (!meta) return;

    setProvider((prev) => ({
      ...prev,
      serverUrl: presetId === 'custom' ? prev.serverUrl : meta.defaultServerUrl,
      remoteRoot: meta.defaultRemoteRoot || prev.remoteRoot,
    }));
    setTestSuccess(null);
    setTestError(null);
    setValidationError(null);
  };
  const updateS3Provider = (patch: Partial<S3ObjectStoreConfig>) => {
    setS3Provider((prev) => {
      const next = { ...prev, ...patch };
      if (patch.endpoint !== undefined) {
        const detected = detectS3ProviderPresetId(patch.endpoint);
        if (detected !== selectedS3PresetId) {
          setSelectedS3PresetId(detected);
        }
      }
      return next;
    });
    setTestSuccess(null);
    setTestError(null);
    setValidationError(null);
  };

  const handleS3PresetChange = (id: string) => {
    const presetId = id as WellKnownS3ProviderId;
    setSelectedS3PresetId(presetId);
    const meta = S3_PROVIDER_PRESETS.find((p) => p.id === presetId);
    if (!meta) return;

    setS3Provider((prev) => ({
      ...prev,
      endpoint: presetId === 's3-custom' ? prev.endpoint : meta.defaultEndpoint,
      region: meta.defaultRegion || prev.region,
      bucket: meta.defaultBucket || prev.bucket,
      remoteRoot: meta.defaultRemoteRoot || prev.remoteRoot,
      forcePathStyle: meta.defaultForcePathStyle,
    }));
    setTestSuccess(null);
    setTestError(null);
    setValidationError(null);
  };

  const currentS3PresetMeta = S3_PROVIDER_PRESETS.find((p) => p.id === selectedS3PresetId);

  const s3ProviderOptions: DropdownOption[] = S3_PROVIDER_PRESETS.map((p) => ({
    value: p.id,
    label: t(p.nameKey, { defaultValue: p.defaultName }),
  }));

  const currentPresetMeta = SYNC_PROVIDER_PRESETS.find((p) => p.id === selectedPresetId);

  const providerOptions: DropdownOption[] = SYNC_PROVIDER_PRESETS.map((p) => ({
    value: p.id,
    label: t(p.nameKey, { defaultValue: p.defaultName }),
  }));

  // Test provider connection
  const handleTestConnection = async () => {
    if (protocolType === 's3') {
      const err = checkS3ProviderFields(s3Provider);
      if (err === 'http_not_local' || err === 'unsupported_scheme' || err === 'https') {
        setValidationError(
          t('settings.sync.error_https_required', {
            defaultValue: 'Server URL must use HTTPS or a local/LAN address.',
          })
        );
        return;
      }
      if (err) {
        setValidationError(
          t('settings.sync.validation_fill_all', {
            defaultValue: 'Fill in all provider credentials.',
          })
        );
        return;
      }
    } else {
      const err = checkProviderFields(provider);
      if (err === 'http_not_local' || err === 'unsupported_scheme' || err === 'https') {
        setValidationError(
          t('settings.sync.error_https_required', {
            defaultValue: 'WebDAV server URL must use HTTPS or a local/LAN address.',
          })
        );
        return;
      }
      if (err) {
        setValidationError(
          t('settings.sync.validation_fill_all', {
            defaultValue: 'Fill in all provider credentials.',
          })
        );
        return;
      }
    }

    setValidationError(null);
    setTestError(null);
    setIsTesting(true);
    try {
      const descriptor = await onTestProvider(activeProviderConfig);
      setTestSuccess(descriptor.displayName || (protocolType === 's3' ? 'S3 Storage' : 'WebDAV'));
    } catch (error) {
      setTestSuccess(null);
      const msg = error instanceof Error ? error.message : String(error);
      setTestError(
        msg ||
          t('settings.sync.detect_failed', {
            defaultValue: 'Connection failed. Check credentials and server URL.',
          })
      );
    } finally {
      setIsTesting(false);
    }
  };

  // Pairing code import logic
  const handleApplyPairingToken = () => {
    if (!pairingTokenInput.trim()) return;
    const decoded = decodeSyncPairingToken(pairingTokenInput);
    if (!decoded) {
      setPairingTokenError(
        t('settings.sync.invalid_token', { defaultValue: 'Invalid pairing token format.' })
      );
      return;
    }
    setPairingTokenError(null);
    if (isS3PairingPayload(decoded)) {
      setProtocolType('s3');
      setS3Provider({
        endpoint: decoded.endpoint,
        region: decoded.region,
        bucket: decoded.bucket,
        remoteRoot: decoded.remoteRoot,
        accessKeyId: decoded.accessKeyId,
        secretAccessKey: decoded.secretAccessKey || s3Provider.secretAccessKey,
        forcePathStyle: Boolean(decoded.forcePathStyle),
      });
      setSelectedS3PresetId(detectS3ProviderPresetId(decoded.endpoint));
    } else if (isWebDavPairingPayload(decoded)) {
      setProtocolType('webdav');
      setProvider({
        serverUrl: decoded.serverUrl,
        remoteRoot: decoded.remoteRoot,
        username: decoded.username,
        password: decoded.providerPassword || provider.password,
      });
      setSelectedPresetId(detectProviderPresetId(decoded.serverUrl));
    }
    setPairingTokenInput('');
    setPairingSuccessNotice(
      t('settings.sync.pairing_applied_notice', {
        defaultValue: 'Imported connection parameters from device (Vault: {{vaultId}})',
        vaultId: decoded.vaultId,
      })
    );
  };

  // Main Save and Connect flow
  const handleSaveAndSync = async () => {
    const providerErr =
      protocolType === 's3' ? checkS3ProviderFields(s3Provider) : checkProviderFields(provider);
    if (
      providerErr === 'http_not_local' ||
      providerErr === 'unsupported_scheme' ||
      providerErr === 'https'
    ) {
      setValidationError(
        t('settings.sync.error_https_required', {
          defaultValue: 'Server URL must use HTTPS or a local/LAN address.',
        })
      );
      return;
    }
    if (providerErr) {
      setValidationError(
        t('settings.sync.validation_fill_all', {
          defaultValue: 'Fill in all provider credentials.',
        })
      );
      return;
    }
    if (!masterPassword) {
      setValidationError(
        unlockMethod === 'recovery'
          ? t('settings.sync.validation_recovery_key', {
              defaultValue: 'Enter your emergency recovery key.',
            })
          : t('settings.sync.validation_master_password', {
              defaultValue: 'Enter a master password.',
            })
      );
      return;
    }
    // Only check password confirmation if using master password and not directly joining an explicitly given vault
    if (
      unlockMethod === 'password' &&
      !vaultId.trim() &&
      confirmPassword &&
      masterPassword !== confirmPassword
    ) {
      setValidationError(
        t('settings.sync.validation_password_match', {
          defaultValue: 'The master password confirmation does not match.',
        })
      );
      return;
    }

    setValidationError(null);
    setIsConnecting(true);

    try {
      // If user explicitly specified a vaultId (or imported via pairing code)
      if (vaultId.trim()) {
        await onJoin({
          provider: activeProviderConfig,
          vaultId: vaultId.trim(),
          masterPassword,
        });
        return;
      }

      // Auto-detect flow
      if (onDiscoverVaults) {
        let vaults: DiscoveredVaultSummary[] = [];
        try {
          vaults = await onDiscoverVaults(activeProviderConfig);
        } catch {
          // If discover fails, fallback to create default
          vaults = [];
        }

        if (vaults.length === 0) {
          // No vault on server -> Initialize default
          await onCreate({
            provider: activeProviderConfig,
            preset,
            masterPassword,
            createRecoveryKey,
            vaultId: 'default',
          });
          return;
        }

        if (vaults.length === 1) {
          // Exactly 1 vault -> Automatically join it
          await onJoin({
            provider: activeProviderConfig,
            vaultId: vaults[0].vaultId,
            masterPassword,
          });
          return;
        }

        // Multiple vaults discovered -> Present choice to user
        setDiscoveredVaults(vaults);
        setSelectedVaultToJoin(vaults[0].vaultId);
        setIsCreatingNewVault(false);
        return;
      }

      // Fallback if no discover callback
      await onCreate({
        provider: activeProviderConfig,
        preset,
        masterPassword,
        createRecoveryKey,
        vaultId: 'default',
      });
    } catch {
      // Structured error handled by parent or dialog store
    } finally {
      setIsConnecting(false);
    }
  };

  // Complete multi-vault selection
  const handleConfirmMultiVaultSelection = async () => {
    setIsConnecting(true);
    try {
      if (isCreatingNewVault) {
        await onCreate({
          provider: activeProviderConfig,
          preset,
          masterPassword,
          createRecoveryKey,
          vaultId: vaultId.trim() || undefined,
        });
      } else {
        await onJoin({
          provider: activeProviderConfig,
          vaultId: selectedVaultToJoin,
          masterPassword,
        });
      }
      setDiscoveredVaults(null);
    } catch {
      // Handled by parent
    } finally {
      setIsConnecting(false);
    }
  };

  return (
    <div className="sync-setup-clean">
      {pairingSuccessNotice && (
        <div className="sync-banner-box is-info">
          <CheckCircle2 size={16} />
          <span>{pairingSuccessNotice}</span>
        </div>
      )}

      {/* Section 1: Storage Provider */}
      <SettingsSection
        title={t('settings.sync.section_storage', { defaultValue: 'Cloud Storage Configuration' })}
        description={t('settings.sync.section_storage_desc', {
          defaultValue:
            'Configure your storage endpoint and credentials for encrypted data synchronization.',
        })}
      >
        <div className="sync-pairing-banner">
          <div className="sync-pairing-banner-text">
            <Link2 size={15} />
            <span>
              {t('settings.sync.have_device_hint', {
                defaultValue: 'Have another device already configured?',
              })}
            </span>
          </div>
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => setShowPairingModal(true)}
            disabled={isBusy}
          >
            {t('settings.sync.import_pairing_token', { defaultValue: 'Import pairing code' })}
          </button>
        </div>

        <SettingsItem
          title={t('settings.sync.storage_type_label', { defaultValue: 'Storage protocol' })}
          hint={t('settings.sync.storage_type_hint', {
            defaultValue: 'Choose between S3-compatible bucket storage and WebDAV cloud storage',
          })}
        >
          <div className="sync-segmented-control" role="tablist">
            <button
              id="sync-type-s3-btn"
              type="button"
              className={`sync-segmented-btn ${protocolType === 's3' ? 'is-active' : ''}`}
              onClick={() => {
                setProtocolType('s3');
                setTestSuccess(null);
                setTestError(null);
                setValidationError(null);
              }}
            >
              <DatabaseZap size={14} style={{ marginRight: '6px' }} />
              {t('settings.sync.protocol_s3', { defaultValue: 'Object Storage Bucket (S3)' })}
            </button>
            <button
              id="sync-type-webdav-btn"
              type="button"
              className={`sync-segmented-btn ${protocolType === 'webdav' ? 'is-active' : ''}`}
              onClick={() => {
                setProtocolType('webdav');
                setTestSuccess(null);
                setTestError(null);
                setValidationError(null);
              }}
            >
              <Server size={14} style={{ marginRight: '6px' }} />
              {t('settings.sync.protocol_webdav', { defaultValue: 'WebDAV' })}
            </button>
          </div>
        </SettingsItem>

        {protocolType === 's3' ? (
          <>
            <SettingsItem
              title={t('settings.sync.choose_s3_provider_label', {
                defaultValue: 'Bucket provider',
              })}
              hint={
                currentS3PresetMeta?.authDocUrl ? (
                  <a
                    href={currentS3PresetMeta.authDocUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="sync-auth-link"
                  >
                    <span>
                      {t('settings.sync.view_auth_guide', { defaultValue: 'View setup guide' })}
                    </span>
                    <ExternalLink size={12} />
                  </a>
                ) : undefined
              }
            >
              <Dropdown
                id="sync-s3-provider-preset"
                value={selectedS3PresetId}
                onChange={handleS3PresetChange}
                options={s3ProviderOptions}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.s3_endpoint', { defaultValue: 'API Endpoint URL' })}
              hint={t('settings.sync.s3_endpoint_hint', {
                defaultValue:
                  'S3-compatible API endpoint URL (e.g. https://<account_id>.r2.cloudflarestorage.com)',
              })}
            >
              <input
                id="sync-s3-endpoint"
                className="settings-input"
                type="url"
                aria-label={t('settings.sync.s3_endpoint', { defaultValue: 'API Endpoint URL' })}
                placeholder="https://s3.amazonaws.com"
                value={s3Provider.endpoint}
                onChange={(e) => updateS3Provider({ endpoint: e.target.value })}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.s3_bucket', { defaultValue: 'Bucket name' })}
              hint={t('settings.sync.s3_bucket_hint', {
                defaultValue: 'The S3 bucket where Sona will store encrypted sync vaults',
              })}
            >
              <input
                id="sync-s3-bucket"
                className="settings-input"
                type="text"
                aria-label={t('settings.sync.s3_bucket', { defaultValue: 'Bucket name' })}
                placeholder="sona-sync"
                value={s3Provider.bucket}
                onChange={(e) => updateS3Provider({ bucket: e.target.value })}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.s3_region', { defaultValue: 'Region' })}
              hint={t('settings.sync.s3_region_hint', {
                defaultValue: 'Bucket region (use "auto" for Cloudflare R2)',
              })}
            >
              <input
                id="sync-s3-region"
                className="settings-input"
                type="text"
                aria-label={t('settings.sync.s3_region', { defaultValue: 'Region' })}
                placeholder="us-east-1"
                value={s3Provider.region}
                onChange={(e) => updateS3Provider({ region: e.target.value })}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.s3_access_key_id', { defaultValue: 'Access Key ID' })}
            >
              <input
                id="sync-s3-ak"
                className="settings-input"
                type="text"
                aria-label={t('settings.sync.s3_access_key_id', { defaultValue: 'Access Key ID' })}
                placeholder={currentS3PresetMeta?.accessKeyPlaceholder || 'Access Key ID'}
                value={s3Provider.accessKeyId}
                onChange={(e) => updateS3Provider({ accessKeyId: e.target.value })}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.s3_secret_access_key', {
                defaultValue: 'Secret Access Key',
              })}
              hint={
                currentS3PresetMeta
                  ? t(currentS3PresetMeta.helpKey, {
                      defaultValue: currentS3PresetMeta.helpDefault,
                    })
                  : undefined
              }
            >
              <div
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '6px',
                  width: '100%',
                  maxWidth: '380px',
                }}
              >
                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                  <PasswordInput
                    id="sync-s3-sk"
                    ariaLabel={t('settings.sync.s3_secret_access_key', {
                      defaultValue: 'Secret Access Key',
                    })}
                    value={s3Provider.secretAccessKey}
                    onChange={(e) => updateS3Provider({ secretAccessKey: e.target.value })}
                    disabled={isBusy}
                    style={{ flex: 1 }}
                  />
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleTestConnection}
                    disabled={isBusy || isTesting}
                    style={{ minWidth: '110px', justifyContent: 'center', whiteSpace: 'nowrap' }}
                  >
                    {isTesting ? (
                      <RefreshCw size={14} className="queue-icon-spin" />
                    ) : testSuccess ? (
                      <CheckCircle2 size={14} style={{ color: 'var(--color-success, #228b4e)' }} />
                    ) : (
                      <Server size={14} />
                    )}
                    <span>
                      {testSuccess
                        ? t('common.connected', { defaultValue: 'Connected' })
                        : t('settings.sync.test_provider_btn', { defaultValue: 'Test connection' })}
                    </span>
                  </button>
                </div>
                {testError && (
                  <div
                    className="sync-banner-box is-error"
                    role="alert"
                    style={{ padding: '6px 10px', marginTop: '2px' }}
                  >
                    <AlertCircle size={14} />
                    <span style={{ fontSize: '0.8rem' }}>{testError}</span>
                  </div>
                )}
              </div>
            </SettingsItem>
          </>
        ) : (
          <>
            <SettingsItem
              title={t('settings.sync.choose_provider_label', { defaultValue: 'Storage provider' })}
              hint={
                currentPresetMeta?.authDocUrl ? (
                  <a
                    href={currentPresetMeta.authDocUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="sync-auth-link"
                  >
                    <span>
                      {t('settings.sync.view_auth_guide', { defaultValue: 'View setup guide' })}
                    </span>
                    <ExternalLink size={12} />
                  </a>
                ) : undefined
              }
            >
              <Dropdown
                id="sync-provider-preset"
                value={selectedPresetId}
                onChange={handlePresetChange}
                options={providerOptions}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.server_url', { defaultValue: 'Server URL' })}
              hint={t('settings.sync.server_url_hint', {
                defaultValue: 'WebDAV endpoint URL (HTTPS or local/LAN HTTP)',
              })}
            >
              <input
                id="sync-server-url"
                className="settings-input"
                type="url"
                aria-label={t('settings.sync.server_url', { defaultValue: 'Server URL' })}
                placeholder="https://dav.example.com/remote.php/dav/files/you/"
                value={provider.serverUrl}
                onChange={(e) => updateProvider({ serverUrl: e.target.value })}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem title={t('settings.sync.username', { defaultValue: 'Username' })}>
              <input
                id="sync-username"
                className="settings-input"
                type="text"
                aria-label={t('settings.sync.username', { defaultValue: 'Username' })}
                placeholder={currentPresetMeta?.usernamePlaceholder || 'username'}
                value={provider.username}
                onChange={(e) => updateProvider({ username: e.target.value })}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            <SettingsItem
              title={t('settings.sync.password', { defaultValue: 'Password' })}
              hint={
                currentPresetMeta
                  ? t(currentPresetMeta.helpKey, { defaultValue: currentPresetMeta.helpDefault })
                  : undefined
              }
            >
              <div
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '6px',
                  width: '100%',
                  maxWidth: '380px',
                }}
              >
                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                  <PasswordInput
                    id="sync-password"
                    ariaLabel={t('settings.sync.password', { defaultValue: 'Password' })}
                    value={provider.password}
                    onChange={(e) => updateProvider({ password: e.target.value })}
                    disabled={isBusy}
                    style={{ flex: 1 }}
                  />
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleTestConnection}
                    disabled={isBusy || isTesting}
                    style={{ minWidth: '110px', justifyContent: 'center', whiteSpace: 'nowrap' }}
                  >
                    {isTesting ? (
                      <RefreshCw size={14} className="queue-icon-spin" />
                    ) : testSuccess ? (
                      <CheckCircle2 size={14} style={{ color: 'var(--color-success, #228b4e)' }} />
                    ) : (
                      <Server size={14} />
                    )}
                    <span>
                      {testSuccess
                        ? t('common.connected', { defaultValue: 'Connected' })
                        : t('settings.sync.test_provider_btn', { defaultValue: 'Test connection' })}
                    </span>
                  </button>
                </div>
                {testError && (
                  <div
                    className="sync-banner-box is-error"
                    role="alert"
                    style={{ padding: '6px 10px', marginTop: '2px' }}
                  >
                    <AlertCircle size={14} />
                    <span style={{ fontSize: '0.8rem' }}>{testError}</span>
                  </div>
                )}
              </div>
            </SettingsItem>
          </>
        )}
      </SettingsSection>

      {/* Section 2: End-to-End Encryption */}
      <SettingsSection
        title={t('settings.sync.section_encryption', { defaultValue: 'End-to-End Encryption' })}
        description={t('settings.sync.section_encryption_desc', {
          defaultValue:
            'Your transcripts and data are encrypted locally before uploading. The storage provider cannot read them.',
        })}
      >
        <SettingsItem
          title={t('settings.sync.unlock_mode', { defaultValue: 'Authentication method' })}
          hint={t('settings.sync.unlock_mode_hint', {
            defaultValue: 'Use master password for encryption, or existing recovery key to decrypt',
          })}
        >
          <div className="sync-segmented-control">
            <button
              type="button"
              className={`sync-segmented-btn ${unlockMethod === 'password' ? 'is-active' : ''}`}
              onClick={() => setUnlockMethod('password')}
            >
              <KeyRound size={13} />
              <span>
                {t('settings.sync.use_master_password', { defaultValue: 'Master password' })}
              </span>
            </button>
            <button
              type="button"
              className={`sync-segmented-btn ${unlockMethod === 'recovery' ? 'is-active' : ''}`}
              onClick={() => setUnlockMethod('recovery')}
            >
              <ShieldCheck size={13} />
              <span>
                {t('settings.sync.use_recovery_key', { defaultValue: 'Emergency recovery key' })}
              </span>
            </button>
          </div>
        </SettingsItem>

        {unlockMethod === 'password' ? (
          <>
            <SettingsItem
              title={t('settings.sync.master_password', { defaultValue: 'Master password' })}
              hint={t('settings.sync.master_password_hint', {
                defaultValue:
                  'Encrypts your sync data. All devices must use this password to unlock and sync.',
              })}
            >
              <PasswordInput
                id="sync-master-password"
                ariaLabel={t('settings.sync.master_password', { defaultValue: 'Master password' })}
                value={masterPassword}
                onChange={(e) => setMasterPassword(e.target.value)}
                disabled={isBusy}
                style={{ width: '100%', maxWidth: '380px' }}
              />
            </SettingsItem>

            {!vaultId.trim() && (
              <SettingsItem
                title={t('settings.sync.confirm_password', { defaultValue: 'Confirm password' })}
                hint={t('settings.sync.confirm_password_hint', {
                  defaultValue: 'Re-enter your master password',
                })}
              >
                <PasswordInput
                  id="sync-confirm-password"
                  ariaLabel={t('settings.sync.confirm_password', {
                    defaultValue: 'Confirm password',
                  })}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  disabled={isBusy}
                  style={{ width: '100%', maxWidth: '380px' }}
                />
              </SettingsItem>
            )}
          </>
        ) : (
          <SettingsItem
            title={t('settings.sync.use_recovery_key', { defaultValue: 'Emergency recovery key' })}
            hint={t('settings.sync.recovery_key_join_hint', {
              defaultValue:
                'If you forgot the master password, you can enter the emergency recovery key saved when this vault was created to decrypt and join.',
            })}
          >
            <input
              id="sync-recovery-key-input"
              className="settings-input sync-monospace-input"
              type="text"
              aria-label={t('settings.sync.use_recovery_key', {
                defaultValue: 'Emergency recovery key',
              })}
              placeholder={t('settings.sync.recovery_key_placeholder', {
                defaultValue: 'Paste the recovery key saved during vault creation',
              })}
              value={masterPassword}
              onChange={(e) => setMasterPassword(e.target.value)}
              disabled={isBusy}
              style={{ width: '100%', maxWidth: '380px' }}
            />
          </SettingsItem>
        )}
      </SettingsSection>

      {/* Section 3: Advanced Settings (Native Sona Accordion) */}
      <SettingsSection>
        <SettingsAccordion
          title={
            <div className="settings-accordion-copy">
              <div className="settings-accordion-copy-title">
                {t('settings.sync.advanced_settings_title', { defaultValue: 'Advanced Settings' })}
              </div>
              <div className="settings-accordion-copy-hint">
                {t('settings.sync.advanced_options', {
                  defaultValue: 'Remote directory, Vault ID, Sync scope, Recovery key',
                })}
              </div>
            </div>
          }
        >
          <SettingsItem
            title={t('settings.sync.remote_root', { defaultValue: 'Remote root' })}
            hint={t('settings.sync.remote_root_hint', {
              defaultValue: 'Folder name on the remote storage, default is Sona',
            })}
          >
            <input
              id="sync-remote-root"
              className="settings-input"
              type="text"
              aria-label={t('settings.sync.remote_root', { defaultValue: 'Remote root' })}
              value={protocolType === 's3' ? s3Provider.remoteRoot : provider.remoteRoot}
              onChange={(e) => {
                if (protocolType === 's3') {
                  updateS3Provider({ remoteRoot: e.target.value });
                } else {
                  updateProvider({ remoteRoot: e.target.value });
                }
              }}
              disabled={isBusy}
              style={{ width: '100%', maxWidth: '380px' }}
            />
          </SettingsItem>
          {protocolType === 's3' && (
            <SettingsItem
              title={t('settings.sync.s3_force_path_style', {
                defaultValue: 'Force Path-style URLs',
              })}
              hint={t('settings.sync.s3_force_path_style_hint', {
                defaultValue:
                  'Required for MinIO and private IP gateways; public cloud providers generally leave this off',
              })}
            >
              <Switch
                id="sync-s3-force-path-style"
                checked={Boolean(s3Provider.forcePathStyle)}
                onChange={(val) => updateS3Provider({ forcePathStyle: val })}
                disabled={isBusy}
              />
            </SettingsItem>
          )}

          <SettingsItem
            title={t('settings.sync.vault_id_custom', { defaultValue: 'Target Vault ID' })}
            hint={t('settings.sync.vault_id_custom_hint', {
              defaultValue: 'Leave empty for auto-discovery or default vault ("default")',
            })}
          >
            <input
              id="sync-vault-id"
              className="settings-input sync-monospace-input"
              type="text"
              aria-label={t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}
              placeholder="default"
              value={vaultId}
              onChange={(e) => setVaultId(e.target.value)}
              disabled={isBusy}
              style={{ width: '100%', maxWidth: '380px' }}
            />
          </SettingsItem>

          <SettingsItem
            title={t('settings.sync.scope_selector_label', { defaultValue: 'Sync scope preset' })}
            hint={t('settings.sync.scope_selector_hint', {
              defaultValue: 'Choose which data types are synchronized to other devices',
            })}
            layout="vertical"
          >
            <div
              className="settings-scenario-cards three-columns"
              style={{ width: '100%', padding: 0, background: 'transparent' }}
            >
              {[
                {
                  id: 'content' as const,
                  label: t('settings.sync.preset_content', { defaultValue: 'Content only' }),
                  description: t('settings.sync.scope_content_desc', {
                    defaultValue: 'Transcripts & summaries',
                  }),
                },
                {
                  id: 'standard' as const,
                  label: t('settings.sync.preset_standard', { defaultValue: 'Standard' }),
                  description: t('settings.sync.scope_standard_desc', {
                    defaultValue: 'Recommended for daily sync',
                  }),
                  badge: t('common.recommended', { defaultValue: 'Recommended' }),
                },
                {
                  id: 'full' as const,
                  label: t('settings.sync.preset_full', { defaultValue: 'Full workspace' }),
                  description: t('settings.sync.scope_full_desc', {
                    defaultValue: 'All settings & profiles',
                  }),
                },
              ].map((s) => {
                const isSelected = preset === s.id;
                return (
                  <button
                    key={s.id}
                    type="button"
                    className={`settings-scenario-card${isSelected ? ' active' : ''}`}
                    onClick={() => setPreset(s.id)}
                    disabled={isBusy}
                  >
                    <span className="settings-scenario-card-icon">
                      <Layers size={18} />
                    </span>
                    <span className="settings-scenario-card-text">
                      <span className="settings-scenario-card-label">
                        {s.label}
                        {s.badge && (
                          <span className="sync-scope-tag is-badge" style={{ marginLeft: '6px' }}>
                            {s.badge}
                          </span>
                        )}
                      </span>
                      <span className="settings-scenario-card-description">{s.description}</span>
                    </span>
                  </button>
                );
              })}
            </div>
          </SettingsItem>
          <SettingsItem
            title={t('settings.sync.create_recovery_key_label', {
              defaultValue: 'Emergency Recovery Key',
            })}
            hint={t('settings.sync.create_recovery_key_hint', {
              defaultValue:
                'Generate a recovery key to restore access if you forget your master password.',
            })}
          >
            <Switch
              id="sync-create-recovery-key"
              aria-label={t('settings.sync.create_recovery_key_label', {
                defaultValue: 'Emergency Recovery Key',
              })}
              checked={createRecoveryKey}
              onChange={(checked) => setCreateRecoveryKey(checked)}
              disabled={isBusy}
            />
          </SettingsItem>
        </SettingsAccordion>
      </SettingsSection>

      {/* Submit Action Footer */}
      <div className="sync-setup-footer">
        {validationError && (
          <div
            className="sync-banner-box is-warning"
            role="alert"
            style={{ width: '100%', maxWidth: '420px' }}
          >
            <AlertCircle size={16} />
            <span>{validationError}</span>
          </div>
        )}

        <button
          type="button"
          className="btn btn-primary sync-save-btn"
          onClick={handleSaveAndSync}
          disabled={isBusy}
        >
          {isConnecting ? (
            <>
              <RefreshCw size={16} className="queue-icon-spin" />
              <span>
                {t('settings.sync.connecting_and_detecting', {
                  defaultValue: 'Connecting and detecting storage...',
                })}
              </span>
            </>
          ) : (
            <>
              <CheckCircle2 size={16} />
              <span>
                {t('settings.sync.save_and_enable', { defaultValue: 'Save & Enable Sync' })}
              </span>
            </>
          )}
        </button>
      </div>

      {/* Pairing Code Import Modal */}
      <Modal
        isOpen={showPairingModal}
        onClose={() => {
          setShowPairingModal(false);
          setPairingTokenError(null);
        }}
        title={t('settings.sync.pairing_modal_title', { defaultValue: 'Import Pairing Code' })}
        size="md"
        footer={
          <>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => {
                setShowPairingModal(false);
                setPairingTokenError(null);
              }}
            >
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={handleApplyPairingToken}
              disabled={!pairingTokenInput.trim()}
            >
              <CheckCircle2 size={15} />
              <span>{t('common.import', { defaultValue: 'Import' })}</span>
            </button>
          </>
        }
      >
        <div className="sync-pairing-modal-body">
          <p className="sync-pairing-modal-desc">
            {t('settings.sync.pairing_modal_desc', {
              defaultValue:
                'Paste the pairing code (sonasync://...) generated on your other device to quickly fill connection parameters.',
            })}
          </p>

          <div className="sync-import-token-box">
            <div className="sync-import-token-header">
              <span className="sync-import-token-label">
                <Link2 size={13} />
                {t('settings.sync.pairing_token_label', { defaultValue: 'Pairing code' })}
              </span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={async () => {
                  try {
                    const text = await navigator.clipboard.readText();
                    if (text) {
                      setPairingTokenInput(text.trim());
                      setPairingTokenError(null);
                    }
                  } catch {
                    // Clipboard access was denied or unavailable
                  }
                }}
              >
                <ClipboardPaste size={13} />
                <span>{t('common.paste', { defaultValue: 'Paste' })}</span>
              </button>
            </div>
            <textarea
              autoFocus
              className="settings-input sync-monospace-input sync-token-textarea"
              rows={4}
              placeholder="sonasync://v1?data=eyJ..."
              value={pairingTokenInput}
              onChange={(e) => {
                setPairingTokenInput(e.target.value);
                setPairingTokenError(null);
              }}
            />
          </div>

          {pairingTokenError && (
            <div className="sync-banner-box is-error" role="alert">
              <AlertCircle size={15} />
              <span>{pairingTokenError}</span>
            </div>
          )}

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

      {/* Multi-Vault Resolution Modal */}
      <Modal
        isOpen={discoveredVaults !== null && discoveredVaults.length > 1}
        onClose={() => setDiscoveredVaults(null)}
        title={t('settings.sync.multi_vault_detected_title', {
          defaultValue: 'Multiple Sync Vaults Detected',
        })}
        size="md"
        footer={
          <>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setDiscoveredVaults(null)}
            >
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={handleConfirmMultiVaultSelection}
              disabled={isConnecting}
            >
              {isConnecting ? (
                <>
                  <RefreshCw size={15} className="queue-icon-spin" />
                  <span>{t('common.connecting', { defaultValue: 'Connecting...' })}</span>
                </>
              ) : (
                <>
                  <Check size={15} />
                  <span>{t('common.confirm', { defaultValue: 'Confirm' })}</span>
                </>
              )}
            </button>
          </>
        }
      >
        <div className="sync-multivault-modal-body">
          <p className="sync-multivault-modal-desc">
            {t('settings.sync.multi_vault_detected_desc', {
              defaultValue:
                'This WebDAV server already contains multiple sync vaults. Choose which vault to connect to, or initialize a new one:',
            })}
          </p>

          <div className="sync-multivault-options-list">
            {discoveredVaults?.map((v) => {
              const isSelected = !isCreatingNewVault && selectedVaultToJoin === v.vaultId;
              return (
                <label
                  key={v.vaultId}
                  className={`sync-multivault-option-card ${isSelected ? 'is-selected' : ''}`}
                >
                  <input
                    type="radio"
                    name="vault-selection"
                    className="sync-radio-input"
                    checked={isSelected}
                    onChange={() => {
                      setSelectedVaultToJoin(v.vaultId);
                      setIsCreatingNewVault(false);
                    }}
                  />
                  <div className="sync-multivault-icon-col">
                    <DatabaseZap size={18} className={isSelected ? 'is-active-icon' : ''} />
                  </div>
                  <div className="sync-multivault-option-info">
                    <div className="sync-multivault-option-title-row">
                      <strong className="sync-multivault-id">{v.vaultId}</strong>
                      <span className="sync-multivault-tag">
                        {t(`settings.sync.preset_${v.preset}`, { defaultValue: v.preset })}
                      </span>
                    </div>
                    <span className="sync-multivault-hint">
                      {v.vaultId === 'default'
                        ? t('settings.sync.vault_default_hint', { defaultValue: 'Default vault' })
                        : t('settings.sync.existing_vault_hint', {
                            defaultValue: 'Existing remote vault',
                          })}
                    </span>
                  </div>
                </label>
              );
            })}

            <label
              className={`sync-multivault-option-card ${isCreatingNewVault ? 'is-selected' : ''}`}
            >
              <input
                type="radio"
                name="vault-selection"
                className="sync-radio-input"
                checked={isCreatingNewVault}
                onChange={() => setIsCreatingNewVault(true)}
              />
              <div className="sync-multivault-icon-col">
                <Sparkles size={18} className={isCreatingNewVault ? 'is-active-icon' : ''} />
              </div>
              <div className="sync-multivault-option-info">
                <div className="sync-multivault-option-title-row">
                  <strong>
                    {t('settings.sync.create_new_vault_option', {
                      defaultValue: 'Create a new independent vault',
                    })}
                  </strong>
                </div>
                <span className="sync-multivault-hint">
                  {t('settings.sync.create_new_vault_option_desc', {
                    defaultValue: 'Initialize a separate workspace on this storage',
                  })}
                </span>
              </div>
            </label>
          </div>
        </div>
      </Modal>
    </div>
  );
}

export default SyncSetupPanel;
