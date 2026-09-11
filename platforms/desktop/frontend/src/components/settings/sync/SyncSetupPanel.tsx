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
  DiscoveredVaultSummary,
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
  SYNC_PROVIDER_PRESETS,
  type WellKnownSyncProviderId,
} from './SyncProviderPresets';
import { decodeSyncPairingToken } from './syncPairing';
export interface SyncSetupPanelProps {
  busyAction: string | null;
  onCreate: (request: SyncCreateRequest) => Promise<SyncCreateResult>;
  onJoin: (request: SyncJoinRequest) => Promise<SyncRunResult>;
  onPreviewJoin: (request: SyncPreviewJoinRequest) => Promise<SyncJoinPreview>;
  onTestProvider: (config: WebDavObjectStoreConfig) => Promise<SyncProviderDescriptor>;
  onDiscoverVaults?: (config: WebDavObjectStoreConfig) => Promise<DiscoveredVaultSummary[]>;
}

function checkProviderFields(config: WebDavObjectStoreConfig): string | null {
  try {
    const url = new URL(config.serverUrl.trim());
    if (url.protocol !== 'https:') {
      return 'https';
    }
  } catch {
    return 'invalid';
  }
  if (!config.remoteRoot.trim() || !config.username.trim() || !config.password) {
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
}: SyncSetupPanelProps): React.JSX.Element {
  const { t } = useTranslation();

  // Form states
  const [selectedPresetId, setSelectedPresetId] =
    React.useState<WellKnownSyncProviderId>('nutstore');
  const [provider, setProvider] = React.useState<WebDavObjectStoreConfig>({
    serverUrl: SYNC_PROVIDER_PRESETS[0].defaultServerUrl,
    remoteRoot: 'Sona',
    username: '',
    password: '',
  });
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

  const currentPresetMeta = SYNC_PROVIDER_PRESETS.find((p) => p.id === selectedPresetId);

  const providerOptions: DropdownOption[] = SYNC_PROVIDER_PRESETS.map((p) => ({
    value: p.id,
    label: t(p.nameKey, { defaultValue: p.defaultName }),
  }));

  // Test provider connection
  const handleTestConnection = async () => {
    const err = checkProviderFields(provider);
    if (err === 'https') {
      setValidationError(
        t('settings.sync.error_https_required', {
          defaultValue: 'WebDAV server URL must use HTTPS.',
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

    setValidationError(null);
    setTestError(null);
    setIsTesting(true);
    try {
      const descriptor = await onTestProvider(provider);
      setTestSuccess(descriptor.displayName || 'WebDAV');
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

    setProvider({
      serverUrl: decoded.serverUrl,
      remoteRoot: decoded.remoteRoot,
      username: decoded.username,
      password: decoded.providerPassword || provider.password,
    });
    setVaultId(decoded.vaultId);
    setSelectedPresetId(detectProviderPresetId(decoded.serverUrl));
    setShowPairingModal(false);
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
    const providerErr = checkProviderFields(provider);
    if (providerErr === 'https') {
      setValidationError(
        t('settings.sync.error_https_required', {
          defaultValue: 'WebDAV server URL must use HTTPS.',
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
          provider,
          vaultId: vaultId.trim(),
          masterPassword,
        });
        return;
      }

      // Auto-detect flow
      if (onDiscoverVaults) {
        let vaults: DiscoveredVaultSummary[] = [];
        try {
          vaults = await onDiscoverVaults(provider);
        } catch {
          // If discover fails, fallback to create default
          vaults = [];
        }

        if (vaults.length === 0) {
          // No vault on server -> Initialize default
          await onCreate({
            provider,
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
            provider,
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
        provider,
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
          provider,
          preset,
          masterPassword,
          createRecoveryKey,
          vaultId: vaultId.trim() || undefined,
        });
      } else {
        await onJoin({
          provider,
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
        title={t('settings.sync.section_storage', { defaultValue: 'WebDAV Storage Configuration' })}
        description={t('settings.sync.section_storage_desc', {
          defaultValue:
            'Configure your WebDAV server endpoint and credentials for encrypted data synchronization.',
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
          hint={t('settings.sync.server_url_hint', { defaultValue: 'HTTPS WebDAV endpoint' })}
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
              value={provider.remoteRoot}
              onChange={(e) => updateProvider({ remoteRoot: e.target.value })}
              disabled={isBusy}
              style={{ width: '100%', maxWidth: '380px' }}
            />
          </SettingsItem>

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
