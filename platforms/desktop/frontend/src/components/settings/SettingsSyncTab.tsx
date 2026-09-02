import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCircle2,
  Cloud,
  CloudOff,
  Copy,
  DatabaseZap,
  Download,
  HelpCircle,
  KeyRound,
  Layers,
  Link2,
  Lock,
  Pause,
  Play,
  QrCode,
  RefreshCw,
  Server,
  ShieldCheck,
  Sparkles,
  Unplug,
} from 'lucide-react';
import {
  changeSyncMasterPassword,
  changeSyncPreset,
  createSyncVault,
  disconnectSyncVault,
  generateSyncRecoveryKey,
  joinSyncVault,
  lockSyncVault,
  previewSyncJoin,
  runSyncNow,
  setSyncPaused,
  testWebDavSyncProvider,
  unlockSyncVault,
  unlockSyncVaultWithRecovery,
} from '../../services/tauri/sync';
import { syncRuntimeService } from '../../services/syncRuntimeService';
import { writeFile } from '../../services/tauri/platform/fs';
import { saveDialog } from '../../services/tauri/platform/dialog';
import { useDialogStore } from '../../stores/dialogStore';
import { useSetConfig, useUIConfig } from '../../stores/configStore';
import { useSyncStatusStore } from '../../stores/syncStatusStore';
import type {
  SyncJoinPreview,
  SyncPresetV1,
  WebDavObjectStoreConfig,
} from '../../types/sync';
import {
  SettingsAccordion,
  SettingsItem,
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
} from './SettingsLayout';
import { Switch } from '../Switch';
import { Modal } from '../Modal';
import {
  SYNC_PROVIDER_PRESETS,
  type WellKnownSyncProviderId,
  detectProviderPresetId,
} from './sync/SyncProviderPresets';
import { SyncConflictCenter } from './sync/SyncConflictCenter';
import './sync/SyncSettings.css';

interface SettingsSyncTabProps {
  isVisible?: boolean;
  isPrewarming?: boolean;
}

const EMPTY_PROVIDER: WebDavObjectStoreConfig = {
  serverUrl: '',
  remoteRoot: 'Sona',
  username: '',
  password: '',
};

function encodeSyncPairingToken(
  provider: WebDavObjectStoreConfig,
  vaultId: string,
): string {
  try {
    const payload = {
      v: 1,
      serverUrl: provider.serverUrl,
      remoteRoot: provider.remoteRoot,
      username: provider.username,
      vaultId,
    };
    const json = JSON.stringify(payload);
    const b64 = btoa(encodeURIComponent(json));
    return `sonasync://v1?data=${b64}`;
  } catch {
    return '';
  }
}

function decodeSyncPairingToken(token: string): Partial<WebDavObjectStoreConfig & { vaultId: string }> | null {
  try {
    const trimmed = token.trim();
    if (!trimmed.startsWith('sonasync://')) return null;
    const url = new URL(trimmed);
    const data = url.searchParams.get('data');
    if (!data) return null;
    const json = decodeURIComponent(atob(data));
    const parsed = JSON.parse(json);
    if (parsed && typeof parsed === 'object') {
      return {
        serverUrl: parsed.serverUrl || '',
        remoteRoot: parsed.remoteRoot || 'Sona',
        username: parsed.username || '',
        vaultId: parsed.vaultId || '',
      };
    }
    return null;
  } catch {
    return null;
  }
}

export function SettingsSyncTab({
  isVisible = true,
  isPrewarming = false,
}: SettingsSyncTabProps): React.JSX.Element {
  const { t, i18n } = useTranslation();
  const alert = useDialogStore((state) => state.alert);
  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);
  const setConfig = useSetConfig();
  const enableCloudSync = useUIConfig().enableCloudSync ?? false;
  const status = useSyncStatusStore((state) => state.snapshot);
  const isStatusLoaded = useSyncStatusStore((state) => state.isLoaded);
  const setSnapshot = useSyncStatusStore((state) => state.setSnapshot);
  const setLastRunResult = useSyncStatusStore((state) => state.setLastRunResult);
  const [busyAction, setBusyAction] = React.useState<string | null>(null);
  // Setup mode states
  const [setupMode, setSetupMode] = React.useState<'create' | 'join'>('create');
  const [createStep, setCreateStep] = React.useState<1 | 2>(1);
  const [selectedPresetId, setSelectedPresetId] = React.useState<WellKnownSyncProviderId>('nutstore');
  const [provider, setProvider] = React.useState<WebDavObjectStoreConfig>(() => {
    const initialPreset = SYNC_PROVIDER_PRESETS.find((p) => p.id === 'nutstore') || SYNC_PROVIDER_PRESETS[0];
    return {
      serverUrl: initialPreset.defaultServerUrl,
      remoteRoot: initialPreset.defaultRemoteRoot,
      username: '',
      password: '',
    };
  });
  const [masterPassword, setMasterPassword] = React.useState('');
  const [confirmPassword, setConfirmPassword] = React.useState('');
  const [createScopePreset, setCreateScopePreset] = React.useState<SyncPresetV1>('standard');
  const [generateRecoveryKeyOption, setGenerateRecoveryKeyOption] = React.useState(true);

  const [vaultId, setVaultId] = React.useState('');
  const [pairingTokenInput, setPairingTokenInput] = React.useState('');
  const [preview, setPreview] = React.useState<SyncJoinPreview | null>(null);

  const [testSuccess, setTestSuccess] = React.useState<string | null>(null);
  const [validationError, setValidationError] = React.useState<string | null>(null);
  const [recoveryKey, setRecoveryKey] = React.useState<string | null>(null);

  // Connected mode states
  const [unlockMode, setUnlockMode] = React.useState<'password' | 'recovery'>('password');
  const [unlockProviderPassword, setUnlockProviderPassword] = React.useState('');
  const [unlockMasterPassword, setUnlockMasterPassword] = React.useState('');
  const [unlockRecoveryInput, setUnlockRecoveryInput] = React.useState('');
  const [presetOverride, setPresetOverride] = React.useState<SyncPresetV1 | null>(null);
  const selectedConnectedPreset = presetOverride ?? status.preset ?? 'standard';
  const [currentPassword, setCurrentPassword] = React.useState('');
  const [nextPassword, setNextPassword] = React.useState('');
  const [confirmNextPassword, setConfirmNextPassword] = React.useState('');
  const [passwordError, setPasswordError] = React.useState<string | null>(null);

  const [showPairingModal, setShowPairingModal] = React.useState(false);
  const [copiedToken, setCopiedToken] = React.useState(false);
  const [copiedKey, setCopiedKey] = React.useState(false);

  const isBusy = busyAction !== null;

  React.useEffect(() => {
    if (isVisible || isPrewarming) {
      void syncRuntimeService.refreshStatus();
    }
  }, [isPrewarming, isVisible]);

  const reportError = React.useCallback((action: string, cause: unknown) => showError({
    code: `sync.${action}_failed`,
    messageKey: 'errors.sync.operation_failed',
    cause,
    titleKey: 'settings.sync.error_title',
  }), [showError]);

  const runReturningAction = React.useCallback(async <T,>(
    action: string,
    task: () => Promise<T>,
  ): Promise<T> => {
    setBusyAction(action);
    try {
      return await task();
    } catch (cause) {
      reportError(action, cause);
      throw cause;
    } finally {
      setBusyAction(null);
    }
  }, [reportError, setBusyAction]);

  const runAction = React.useCallback(async (
    action: string,
    task: () => Promise<void>,
  ): Promise<void> => {
    await runReturningAction(action, task);
  }, [runReturningAction]);

  const currentPresetMeta = React.useMemo(() => {
    return SYNC_PROVIDER_PRESETS.find((p) => p.id === selectedPresetId) || SYNC_PROVIDER_PRESETS[0];
  }, [selectedPresetId]);

  const handleSelectPreset = (id: WellKnownSyncProviderId) => {
    setSelectedPresetId(id);
    const meta = SYNC_PROVIDER_PRESETS.find((p) => p.id === id);
    if (meta) {
      setProvider((prev) => ({
        ...prev,
        serverUrl: meta.defaultServerUrl || (id === 'custom' ? '' : prev.serverUrl),
        remoteRoot: meta.defaultRemoteRoot,
      }));
    }
    setTestSuccess(null);
    setValidationError(null);
  };

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
    setValidationError(null);
  };

  const handleTokenChange = (text: string) => {
    setPairingTokenInput(text);
    setValidationError(null);
    const decoded = decodeSyncPairingToken(text);
    if (decoded) {
      if (decoded.serverUrl) updateProvider({ serverUrl: decoded.serverUrl });
      if (decoded.remoteRoot) updateProvider({ remoteRoot: decoded.remoteRoot });
      if (decoded.username) updateProvider({ username: decoded.username });
      if (decoded.vaultId) setVaultId(decoded.vaultId);
      setPreview(null);
    } else {
      if (text.trim().length > 10) {
        setValidationError(t('settings.sync.invalid_token', { defaultValue: 'Invalid pairing token format.' }));
      }
    }
  };

  const runTestConnection = async () => {
    const error = checkProviderFields(provider);
    if (error === 'https') {
      setValidationError(t('settings.sync.validation_https', { defaultValue: 'WebDAV requires an HTTPS server URL.' }));
      return;
    }
    if (error) {
      setValidationError(t('settings.sync.validation_provider', { defaultValue: 'Complete the WebDAV server, username, and password fields.' }));
      return;
    }
    try {
      await runAction('test_provider', async () => {
        const d = await testWebDavSyncProvider(provider);
        setTestSuccess(d.displayName || 'WebDAV');
        setValidationError(null);
        await alert(t('settings.sync.provider_ready', {
          defaultValue: '{{provider}} is ready for sync.',
          provider: d.displayName,
        }), { variant: 'success' });
      });
    } catch {
      setTestSuccess(null);
    }
  };

  const validateStep1 = (): boolean => {
    const error = checkProviderFields(provider);
    if (error === 'https') {
      setValidationError(t('settings.sync.validation_https', { defaultValue: 'WebDAV requires an HTTPS server URL.' }));
      return false;
    }
    if (error) {
      setValidationError(t('settings.sync.validation_provider', { defaultValue: 'Complete the WebDAV server, username, and password fields.' }));
      return false;
    }
    setValidationError(null);
    return true;
  };

  const validateStep2 = (): boolean => {
    if (!masterPassword) {
      setValidationError(t('settings.sync.validation_master_password', { defaultValue: 'Enter a master password.' }));
      return false;
    }
    if (masterPassword !== confirmPassword) {
      setValidationError(t('settings.sync.validation_password_match', { defaultValue: 'The master password confirmation does not match.' }));
      return false;
    }
    setValidationError(null);
    return true;
  };

  const validateJoin = (): boolean => {
    if (!validateStep1()) return false;
    if (!vaultId.trim()) {
      setValidationError(t('settings.sync.validation_vault_id', { defaultValue: 'Enter the vault ID from an existing device.' }));
      return false;
    }
    if (!masterPassword) {
      setValidationError(t('settings.sync.validation_master_password', { defaultValue: 'Enter a master password.' }));
      return false;
    }
    setValidationError(null);
    return true;
  };

  const handleCreateVault = async () => {
    if (!validateStep2()) return;
    await runAction('create', async () => {
      const result = await createSyncVault({
        provider,
        preset: createScopePreset,
        masterPassword,
        createRecoveryKey: generateRecoveryKeyOption,
      });
      setSnapshot(result.status);
      setRecoveryKey(result.recoveryKey);
      await syncRuntimeService.refreshStatus();
    });
  };

  const handlePreviewJoin = async () => {
    if (!validateJoin()) return;
    await runAction('preview_join', async () => {
      const p = await previewSyncJoin({
        provider,
        vaultId: vaultId.trim(),
        masterPassword,
      });
      setPreview(p);
    });
  };

  const handleConfirmJoin = async () => {
    if (!validateJoin()) return;
    await runAction('join', async () => {
      const result = await joinSyncVault({
        provider,
        vaultId: vaultId.trim(),
        masterPassword,
      });
      setLastRunResult(result);
      await syncRuntimeService.refreshStatus();
    });
  };

  const handleUnlock = async () => {
    await runAction('unlock', async () => {
      if (unlockMode === 'password') {
        const next = await unlockSyncVault({
          providerPassword: unlockProviderPassword,
          masterPassword: unlockMasterPassword,
        });
        setSnapshot(next);
      } else {
        const next = await unlockSyncVaultWithRecovery({
          providerPassword: unlockProviderPassword,
          recoveryKey: unlockRecoveryInput.trim(),
        });
        setSnapshot(next);
      }
      await syncRuntimeService.refreshStatus();
    });
  };

  const handleRunNow = () => runAction('sync', async () => {
    const result = await runSyncNow();
    setLastRunResult(result);
    await syncRuntimeService.refreshStatus();
  });

  const handleSetPaused = (paused: boolean) => runAction('pause', async () => {
    const next = await setSyncPaused(paused);
    setSnapshot(next);
    await syncRuntimeService.refreshStatus();
  });

  const handleLock = () => runAction('lock', async () => {
    const next = await lockSyncVault();
    setSnapshot(next);
  });

  const handleChangePreset = (presetValue: SyncPresetV1) => runAction('change_preset', async () => {
    const next = await changeSyncPreset(presetValue, true);
    setSnapshot(next);
  });

  const handleChangeMasterPassword = () => {
    if (nextPassword !== confirmNextPassword) {
      setPasswordError(t('settings.sync.validation_password_match', { defaultValue: 'The new master password confirmation does not match.' }));
      return;
    }
    setPasswordError(null);
    void runAction('change_master_password', async () => {
      await changeSyncMasterPassword({
        currentMasterPassword: currentPassword,
        nextMasterPassword: nextPassword,
      });
      setCurrentPassword('');
      setNextPassword('');
      setConfirmNextPassword('');
      await alert(t('settings.sync.password_changed_success', { defaultValue: 'Master password updated successfully.' }), { variant: 'success' });
    });
  };

  const handleGenerateRecoveryKey = () => runAction('generate_recovery_key', async () => {
    const key = await generateSyncRecoveryKey();
    setRecoveryKey(key);
  });

  const handleExportRecoveryKey = () => runAction('export_recovery_key', async () => {
    if (!recoveryKey) return;
    const outputPath = await saveDialog({
      defaultPath: 'sona-recovery-key.txt',
      filters: [{ name: 'Text file', extensions: ['txt'] }],
    });
    if (outputPath) {
      await writeFile(outputPath, new TextEncoder().encode(`${recoveryKey}\n`));
      await alert(t('settings.sync.recovery_key_exported', { defaultValue: 'Recovery key saved to {{path}}', path: outputPath }), { variant: 'success' });
    }
  });

  const handleDisconnect = async () => {
    const approved = await confirm(
      t('settings.sync.disconnect_confirm_message', {
        defaultValue: 'Disconnect this device from the sync vault? Local data stays intact.',
      }),
      {
        title: t('settings.sync.disconnect_confirm_title', { defaultValue: 'Disconnect Sync Vault' }),
        confirmLabel: t('settings.sync.disconnect', { defaultValue: 'Disconnect' }),
        variant: 'error',
      },
    );
    if (!approved) return;
    await runAction('disconnect', async () => {
      const next = await disconnectSyncVault();
      setSnapshot(next);
      setRecoveryKey(null);
    });
  };

  const pairingToken = React.useMemo(() => {
    if (!status.vaultId) return '';
    return encodeSyncPairingToken(
      provider.serverUrl ? provider : { ...EMPTY_PROVIDER, serverUrl: 'https://...' },
      status.vaultId,
    );
  }, [provider, status.vaultId]);

  const parsedTokenInfo = React.useMemo(() => {
    if (!pairingTokenInput.trim()) return null;
    const decoded = decodeSyncPairingToken(pairingTokenInput);
    if (decoded && decoded.serverUrl) {
      return `${decoded.serverUrl} (${decoded.vaultId ? decoded.vaultId.slice(0, 8) + '...' : ''})`;
    }
    return null;
  }, [pairingTokenInput]);

  return (
    <SettingsTabContainer id="settings-sync-panel" ariaLabelledby="settings-tab-sync">
      <SettingsPageHeader
        icon={<Cloud width={28} height={28} />}
        title={t('settings.sync.title', { defaultValue: 'Cloud Sync' })}
        description={t('settings.sync.page_description', {
          defaultValue: 'End-to-end encrypted synchronization across devices with WebDAV.',
        })}
      />

      {/* Feature Switch Section */}
      <SettingsSection>
        <SettingsItem
          title={t('settings.sync.enable_cloud_sync', { defaultValue: 'Enable Cloud Sync' })}
          hint={t('settings.sync.enable_cloud_sync_hint', {
            defaultValue: 'Enable end-to-end encrypted WebDAV sync across devices and show status capsule in the header.',
          })}
        >
          <Switch
            id="enable-cloud-sync-switch"
            checked={enableCloudSync}
            onChange={(checked) => setConfig({ enableCloudSync: checked })}
          />
        </SettingsItem>
      </SettingsSection>

      {!enableCloudSync ? (
        <div className="sync-disabled-feature-card">
          <div className="sync-disabled-feature-icon">
            <CloudOff size={28} />
          </div>
          <div className="sync-disabled-feature-content">
            <strong>
              {t('settings.sync.disabled_feature_notice', {
                defaultValue: 'Cloud sync is currently turned off. Turn it on to configure WebDAV sync and seamlessly sync transcripts and settings across devices.',
              })}
            </strong>
            <ul className="sync-disabled-feature-benefits">
              <li>
                <ShieldCheck size={16} />
                <span>{t('settings.sync.e2e_encryption_title', { defaultValue: 'End-to-End Encryption' })}</span>
              </li>
              <li>
                <Sparkles size={16} />
                <span>{t('settings.sync.scope_transcripts', { defaultValue: 'Transcripts & History' })}</span>
              </li>
            </ul>
          </div>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => setConfig({ enableCloudSync: true })}
          >
            <Cloud size={16} />
            <span>{t('settings.sync.turn_on_sync', { defaultValue: 'Turn On Cloud Sync' })}</span>
          </button>
        </div>
      ) : !isStatusLoaded ? (
        <SettingsSection>
          <div className="sync-banner-row">
            <div className="sync-banner-box is-info">{t('common.loading', { defaultValue: 'Loading...' })}</div>
          </div>
        </SettingsSection>
      ) : status.state === 'disabled' ? (
        /* Setup Mode (Create or Join) */
        <SettingsSection
          title={t('settings.sync.setup_title', { defaultValue: 'Sync Vault Setup' })}
          description={t('settings.sync.setup_description', {
            defaultValue: 'Initialize a new encrypted sync vault or connect to an existing vault on your WebDAV server.',
          })}
          icon={<ShieldCheck size={20} />}
        >
          {/* Create vs Join scenario cards */}
          <div
            className="settings-scenario-cards"
            role="tablist"
            aria-label={t('settings.sync.setup_mode', { defaultValue: 'Sync setup mode' })}
          >
            {[
              {
                id: 'create' as const,
                icon: <DatabaseZap size={18} />,
                label: t('settings.sync.create_tab', { defaultValue: 'Create vault' }),
                description: t('settings.sync.create_tab_desc', {
                  defaultValue: 'Initialize a new end-to-end encrypted sync vault on your WebDAV server',
                }),
              },
              {
                id: 'join' as const,
                icon: <Link2 size={18} />,
                label: t('settings.sync.join_tab', { defaultValue: 'Join vault' }),
                description: t('settings.sync.join_tab_desc', {
                  defaultValue: 'Connect and sync with an existing vault using a pairing code or server credentials',
                }),
              },
            ].map((option) => (
              <button
                key={option.id}
                type="button"
                role="tab"
                aria-selected={setupMode === option.id}
                className={`settings-scenario-card${setupMode === option.id ? ' active' : ''}`}
                onClick={() => {
                  setSetupMode(option.id);
                  setValidationError(null);
                }}
                disabled={isBusy}
              >
                <span className="settings-scenario-card-icon">
                  {option.icon}
                </span>
                <span className="settings-scenario-card-text">
                  <span className="settings-scenario-card-label">{option.label}</span>
                  <span className="settings-scenario-card-description">{option.description}</span>
                </span>
              </button>
            ))}
          </div>

          {setupMode === 'create' ? (
            <>
              {/* Wizard Step Indicator */}
              <div className="sync-step-indicator">
                <div className={`sync-step-node${createStep === 1 ? ' is-active' : ''}`}>
                  <span className="sync-step-num">1</span>
                  <span>{t('settings.sync.step1_indicator', { defaultValue: '1. Provider & Connection' })}</span>
                </div>
                <div className="sync-step-line" />
                <div className={`sync-step-node${createStep === 2 ? ' is-active' : ''}`}>
                  <span className="sync-step-num">2</span>
                  <span>{t('settings.sync.step2_indicator', { defaultValue: '2. Security & Scope' })}</span>
                </div>
              </div>

              {createStep === 1 ? (
                <>
                  <SettingsItem
                    title={t('settings.sync.choose_provider_label', { defaultValue: 'WebDAV Provider' })}
                    hint={t('settings.sync.choose_provider_hint', { defaultValue: 'Select a pre-configured service template or custom WebDAV' })}
                    layout="vertical"
                  >
                    <div className="settings-scenario-cards three-columns" style={{ width: '100%', padding: 0, background: 'transparent' }}>
                      {SYNC_PROVIDER_PRESETS.map((p) => {
                        const isSelected = selectedPresetId === p.id;
                        return (
                          <button
                            key={p.id}
                            type="button"
                            className={`settings-scenario-card${isSelected ? ' active' : ''}`}
                            onClick={() => handleSelectPreset(p.id)}
                            disabled={isBusy}
                          >
                            <span className="settings-scenario-card-icon">
                              <Server size={18} />
                            </span>
                            <span className="settings-scenario-card-text">
                              <span className="settings-scenario-card-label">
                                {t(p.nameKey, { defaultValue: p.defaultName })}
                              </span>
                              <span className="settings-scenario-card-description">
                                {t(p.badgeKey, { defaultValue: p.defaultBadge })}
                              </span>
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </SettingsItem>

                  {currentPresetMeta && (
                    <div className="sync-banner-row">
                      <div className="sync-banner-box is-info">
                        <HelpCircle size={16} />
                        <div className="sync-banner-content">
                          <p>{t(currentPresetMeta.helpKey, { defaultValue: currentPresetMeta.helpDefault })}</p>
                          {currentPresetMeta.authDocUrl && (
                            <a
                              href={currentPresetMeta.authDocUrl}
                              target="_blank"
                              rel="noreferrer"
                              className="sync-help-link"
                            >
                              {t('settings.sync.view_auth_guide', { defaultValue: 'View setup guide' })} &rarr;
                            </a>
                          )}
                        </div>
                      </div>
                    </div>
                  )}

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
                      style={{ width: '280px' }}
                    />
                  </SettingsItem>

                  <SettingsItem
                    title={t('settings.sync.remote_root', { defaultValue: 'Remote root' })}
                    hint={t('settings.sync.remote_root_hint', { defaultValue: 'Folder name on the remote storage' })}
                  >
                    <input
                      id="sync-remote-root"
                      className="settings-input"
                      type="text"
                      aria-label={t('settings.sync.remote_root', { defaultValue: 'Remote root' })}
                      value={provider.remoteRoot}
                      onChange={(e) => updateProvider({ remoteRoot: e.target.value })}
                      disabled={isBusy}
                      style={{ width: '280px' }}
                    />
                  </SettingsItem>

                  <SettingsItem
                    title={t('settings.sync.username', { defaultValue: 'Username' })}
                    hint={t('settings.sync.username_hint', { defaultValue: 'Account username or email' })}
                  >
                    <input
                      id="sync-username"
                      className="settings-input"
                      type="text"
                      aria-label={t('settings.sync.username', { defaultValue: 'Username' })}
                      placeholder={currentPresetMeta?.usernamePlaceholder || 'username'}
                      value={provider.username}
                      onChange={(e) => updateProvider({ username: e.target.value })}
                      disabled={isBusy}
                      style={{ width: '280px' }}
                    />
                  </SettingsItem>

                  <SettingsItem
                    title={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
                    hint={t('settings.sync.provider_password_hint', { defaultValue: 'Dedicated app password (recommended)' })}
                  >
                    <div className="sync-inline-input-group">
                      <input
                        id="sync-provider-password"
                        className="settings-input"
                        type="password"
                        autoComplete="current-password"
                        aria-label={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
                        placeholder="••••••••••••"
                        value={provider.password}
                        onChange={(e) => updateProvider({ password: e.target.value })}
                        disabled={isBusy}
                        style={{ width: '180px' }}
                      />
                      <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        onClick={runTestConnection}
                        disabled={isBusy}
                      >
                        {busyAction === 'test_provider'
                          ? t('settings.sync.testing', { defaultValue: 'Testing...' })
                          : t('settings.sync.test_provider', { defaultValue: 'Test connection' })}
                      </button>
                    </div>
                  </SettingsItem>

                  {testSuccess && (
                    <div className="sync-banner-row">
                      <div className="sync-banner-box is-success">
                        <CheckCircle2 size={16} />
                        <span>{t('settings.sync.test_passed', { defaultValue: 'Connected successfully to {{name}}', name: testSuccess })}</span>
                      </div>
                    </div>
                  )}

                  {validationError && (
                    <div className="sync-banner-row">
                      <div className="sync-banner-box is-error">
                        <AlertCircle size={16} />
                        <span>{validationError}</span>
                      </div>
                    </div>
                  )}

                  <div className="sync-wizard-actions">
                    <button
                      type="button"
                      className="btn btn-primary"
                      onClick={() => {
                        if (validateStep1()) {
                          setCreateStep(2);
                        }
                      }}
                      disabled={isBusy}
                    >
                      <span>{t('settings.sync.next_step', { defaultValue: 'Next step: Security & Scope' })}</span>
                      <ArrowRight size={16} />
                    </button>
                  </div>
                </>
              ) : (
                /* Create Step 2: Security & Scope */
                <>
                  <div className="sync-banner-row">
                    <div className="sync-banner-box is-security">
                      <ShieldCheck size={16} />
                      <div className="sync-banner-content">
                        <strong>{t('settings.sync.e2e_encryption_title', { defaultValue: 'End-to-End Encryption' })}</strong>
                        <p>{t('settings.sync.e2e_encryption_desc', {
                          defaultValue: 'Your data is encrypted on this device before uploading. Nobody, not even the storage provider, can decrypt your transcripts without the master password.',
                        })}</p>
                      </div>
                    </div>
                  </div>

                  <SettingsItem
                    title={t('settings.sync.master_password', { defaultValue: 'Master password' })}
                    hint={t('settings.sync.master_password_hint', { defaultValue: 'Used to encrypt and unlock the sync vault across your devices' })}
                  >
                    <input
                      id="sync-master-password"
                      className="settings-input"
                      type="password"
                      autoComplete="new-password"
                      aria-label={t('settings.sync.master_password', { defaultValue: 'Master password' })}
                      value={masterPassword}
                      onChange={(e) => setMasterPassword(e.target.value)}
                      disabled={isBusy}
                      style={{ width: '280px' }}
                    />
                  </SettingsItem>

                  <SettingsItem
                    title={t('settings.sync.confirm_password', { defaultValue: 'Confirm master password' })}
                    hint={t('settings.sync.confirm_password_hint', { defaultValue: 'Re-enter your master password to prevent typos' })}
                  >
                    <input
                      id="sync-confirm-password"
                      className="settings-input"
                      type="password"
                      autoComplete="new-password"
                      aria-label={t('settings.sync.confirm_password', { defaultValue: 'Confirm master password' })}
                      value={confirmPassword}
                      onChange={(e) => setConfirmPassword(e.target.value)}
                      disabled={isBusy}
                      style={{ width: '280px' }}
                    />
                  </SettingsItem>

                  <SettingsItem
                    title={t('settings.sync.scope_selector_label', { defaultValue: 'Sync scope preset' })}
                    hint={t('settings.sync.scope_selector_hint', { defaultValue: 'Choose which data types are synchronized to other devices' })}
                    layout="vertical"
                  >
                    <div className="settings-scenario-cards three-columns" style={{ width: '100%', padding: 0, background: 'transparent' }}>
                      {[
                        {
                          id: 'content' as const,
                          label: t('settings.sync.preset_content', { defaultValue: 'Content only' }),
                          description: t('settings.sync.scope_content_desc', { defaultValue: 'Transcripts & summaries' }),
                        },
                        {
                          id: 'standard' as const,
                          label: t('settings.sync.preset_standard', { defaultValue: 'Standard' }),
                          description: t('settings.sync.scope_standard_desc', { defaultValue: 'Recommended for daily sync' }),
                          badge: t('common.recommended', { defaultValue: 'Recommended' }),
                        },
                        {
                          id: 'full' as const,
                          label: t('settings.sync.preset_full', { defaultValue: 'Full workspace' }),
                          description: t('settings.sync.scope_full_desc', { defaultValue: 'All settings & profiles' }),
                        },
                      ].map((s) => {
                        const isSelected = createScopePreset === s.id;
                        return (
                          <button
                            key={s.id}
                            type="button"
                            className={`settings-scenario-card${isSelected ? ' active' : ''}`}
                            onClick={() => setCreateScopePreset(s.id)}
                            disabled={isBusy}
                          >
                            <span className="settings-scenario-card-icon">
                              <Layers size={18} />
                            </span>
                            <span className="settings-scenario-card-text">
                              <span className="settings-scenario-card-label">
                                {s.label}
                                {s.badge && <span className="sync-scope-tag is-badge" style={{ marginLeft: '6px' }}>{s.badge}</span>}
                              </span>
                              <span className="settings-scenario-card-description">
                                {s.description}
                              </span>
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </SettingsItem>

                  <SettingsItem
                    title={t('settings.sync.create_recovery_key', { defaultValue: 'Generate recovery key' })}
                    hint={t('settings.sync.create_recovery_key_hint', { defaultValue: 'Create a backup emergency key to restore access if you forget the master password' })}
                  >
                    <Switch
                      id="sync-create-recovery-key"
                      checked={generateRecoveryKeyOption}
                      onChange={(checked) => setGenerateRecoveryKeyOption(checked)}
                    />
                  </SettingsItem>

                  {validationError && (
                    <div className="sync-banner-row">
                      <div className="sync-banner-box is-error">
                        <AlertCircle size={16} />
                        <span>{validationError}</span>
                      </div>
                    </div>
                  )}

                  <div className="sync-wizard-actions has-back">
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => setCreateStep(1)}
                      disabled={isBusy}
                    >
                      <ArrowLeft size={16} />
                      <span>{t('common.back', { defaultValue: 'Back' })}</span>
                    </button>
                    <button
                      type="button"
                      className="btn btn-primary"
                      onClick={() => void handleCreateVault()}
                      disabled={isBusy}
                    >
                      <DatabaseZap size={16} />
                      <span>
                        {busyAction === 'create'
                          ? t('settings.sync.creating', { defaultValue: 'Creating...' })
                          : t('settings.sync.create_action', { defaultValue: 'Create sync vault' })}
                      </span>
                    </button>
                  </div>
                </>
              )}
            </>
          ) : (
            /* Join Mode */
            <>
              <SettingsItem
                title={t('settings.sync.pairing_token_label', { defaultValue: 'Pairing code (Optional)' })}
                hint={t('settings.sync.pairing_token_hint', { defaultValue: 'Paste the pairing code (sonasync://...) generated on your primary device to auto-fill connection parameters' })}
                layout="vertical"
              >
                <textarea
                  id="sync-pairing-token-input"
                  className="settings-input sync-token-textarea"
                  aria-label={t('settings.sync.pairing_token_label', { defaultValue: 'Pairing code' })}
                  placeholder="sonasync://v1?data=eyJ..."
                  value={pairingTokenInput}
                  onChange={(e) => handleTokenChange(e.target.value)}
                  disabled={isBusy}
                  spellCheck={false}
                />
              </SettingsItem>

              {parsedTokenInfo && (
                <div className="sync-banner-row">
                  <div className="sync-banner-box is-success">
                    <Check size={16} />
                    <span>{t('settings.sync.token_parsed_success', { defaultValue: 'Recognized: {{info}}', info: parsedTokenInfo })}</span>
                  </div>
                </div>
              )}

              <SettingsItem
                title={t('settings.sync.choose_provider_label', { defaultValue: 'WebDAV Provider' })}
                hint={t('settings.sync.choose_provider_hint', { defaultValue: 'Select a pre-configured service template or custom WebDAV' })}
                layout="vertical"
              >
                <div className="settings-scenario-cards three-columns" style={{ width: '100%', padding: 0, background: 'transparent' }}>
                  {SYNC_PROVIDER_PRESETS.map((p) => {
                    const isSelected = selectedPresetId === p.id;
                    return (
                      <button
                        key={p.id}
                        type="button"
                        className={`settings-scenario-card${isSelected ? ' active' : ''}`}
                        onClick={() => handleSelectPreset(p.id)}
                        disabled={isBusy}
                      >
                        <span className="settings-scenario-card-icon">
                          <Server size={18} />
                        </span>
                        <span className="settings-scenario-card-text">
                          <span className="settings-scenario-card-label">
                            {t(p.nameKey, { defaultValue: p.defaultName })}
                          </span>
                          <span className="settings-scenario-card-description">
                            {t(p.badgeKey, { defaultValue: p.defaultBadge })}
                          </span>
                        </span>
                      </button>
                    );
                  })}
                </div>
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
                  style={{ width: '280px' }}
                />
              </SettingsItem>

              <SettingsItem
                title={t('settings.sync.remote_root', { defaultValue: 'Remote root' })}
                hint={t('settings.sync.remote_root_hint', { defaultValue: 'Folder name on the remote storage' })}
              >
                <input
                  id="sync-remote-root"
                  className="settings-input"
                  type="text"
                  aria-label={t('settings.sync.remote_root', { defaultValue: 'Remote root' })}
                  value={provider.remoteRoot}
                  onChange={(e) => updateProvider({ remoteRoot: e.target.value })}
                  disabled={isBusy}
                  style={{ width: '280px' }}
                />
              </SettingsItem>

              <SettingsItem
                title={t('settings.sync.username', { defaultValue: 'Username' })}
                hint={t('settings.sync.username_hint', { defaultValue: 'Account username or email' })}
              >
                <input
                  id="sync-username"
                  className="settings-input"
                  type="text"
                  aria-label={t('settings.sync.username', { defaultValue: 'Username' })}
                  placeholder={currentPresetMeta?.usernamePlaceholder || 'username'}
                  value={provider.username}
                  onChange={(e) => updateProvider({ username: e.target.value })}
                  disabled={isBusy}
                  style={{ width: '280px' }}
                />
              </SettingsItem>

              <SettingsItem
                title={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
                hint={t('settings.sync.provider_password_hint', { defaultValue: 'Dedicated app password' })}
              >
                <input
                  id="sync-provider-password"
                  className="settings-input"
                  type="password"
                  autoComplete="current-password"
                  aria-label={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
                  placeholder="••••••••••••"
                  value={provider.password}
                  onChange={(e) => updateProvider({ password: e.target.value })}
                  disabled={isBusy}
                  style={{ width: '280px' }}
                />
              </SettingsItem>

              <SettingsItem
                title={t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}
                hint={t('settings.sync.vault_id_hint', { defaultValue: 'Find this in the sync settings of your primary device' })}
              >
                <input
                  id="sync-vault-id"
                  className="settings-input"
                  type="text"
                  aria-label={t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}
                  spellCheck={false}
                  value={vaultId}
                  onChange={(e) => {
                    setVaultId(e.target.value);
                    setPreview(null);
                  }}
                  disabled={isBusy}
                  style={{ width: '280px' }}
                />
              </SettingsItem>

              <SettingsItem
                title={t('settings.sync.master_password', { defaultValue: 'Master password' })}
                hint={t('settings.sync.master_password_hint', { defaultValue: 'The password used when creating this vault on the primary device' })}
              >
                <input
                  id="sync-join-master-password"
                  className="settings-input"
                  type="password"
                  autoComplete="current-password"
                  aria-label={t('settings.sync.master_password', { defaultValue: 'Master password' })}
                  value={masterPassword}
                  onChange={(e) => setMasterPassword(e.target.value)}
                  disabled={isBusy}
                  style={{ width: '280px' }}
                />
              </SettingsItem>

              {preview && (
                <div className="sync-preview-wrapper">
                  <div className="sync-preview-card">
                    <div className="sync-preview-header">
                      <Sparkles size={16} />
                      <strong>{t('settings.sync.preview_title', { defaultValue: 'Join preview' })}</strong>
                    </div>
                    <div className="sync-preview-grid">
                      <div>
                        <span>{t('settings.sync.preview_remote', { defaultValue: 'Remote operations' })}</span>
                        <strong>{preview.remoteOperationCount}</strong>
                      </div>
                      <div>
                        <span>{t('settings.sync.preview_conflicts', { defaultValue: 'Projected conflicts' })}</span>
                        <strong>{preview.projectedConflictCount}</strong>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {validationError && (
                <div className="sync-banner-row">
                  <div className="sync-banner-box is-error">
                    <AlertCircle size={16} />
                    <span>{validationError}</span>
                  </div>
                </div>
              )}

              <div className="sync-wizard-actions">
                {preview ? (
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={() => void handleConfirmJoin()}
                    disabled={isBusy}
                  >
                    <Check size={16} />
                    <span>
                      {busyAction === 'join'
                        ? t('settings.sync.joining', { defaultValue: 'Joining...' })
                        : t('settings.sync.confirm_join', { defaultValue: 'Confirm join' })}
                    </span>
                  </button>
                ) : (
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={() => void handlePreviewJoin()}
                    disabled={isBusy}
                  >
                    <Sparkles size={16} />
                    <span>
                      {busyAction === 'preview_join'
                        ? t('settings.sync.previewing', { defaultValue: 'Preparing preview...' })
                        : t('settings.sync.preview_join', { defaultValue: 'Preview join' })}
                    </span>
                  </button>
                )}
              </div>
            </>
          )}
        </SettingsSection>
      ) : status.state === 'locked' ? (
        /* Locked Vault Mode */
        <SettingsSection
          title={t('settings.sync.locked_title', { defaultValue: 'Sync Vault Locked' })}
          description={t('settings.sync.locked_hint', { defaultValue: 'Enter your credentials to unlock and resume synchronization.' })}
          icon={<Lock size={20} />}
        >
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
              value={unlockProviderPassword}
              onChange={(event) => setUnlockProviderPassword(event.target.value)}
              disabled={isBusy}
              style={{ width: '280px' }}
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
                value={unlockMasterPassword}
                onChange={(event) => setUnlockMasterPassword(event.target.value)}
                disabled={isBusy}
                style={{ width: '280px' }}
              />
            </SettingsItem>
          ) : (
            <SettingsItem
              title={t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}
              hint={t('settings.sync.recovery_key_hint', { defaultValue: 'Enter your emergency recovery key' })}
            >
              <input
                id="sync-unlock-recovery-key"
                className="settings-input"
                type="text"
                spellCheck={false}
                aria-label={t('settings.sync.recovery_key', { defaultValue: 'Recovery key' })}
                value={unlockRecoveryInput}
                onChange={(event) => setUnlockRecoveryInput(event.target.value)}
                disabled={isBusy}
                style={{ width: '280px' }}
              />
            </SettingsItem>
          )}

          <div className="sync-wizard-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void handleUnlock()}
              disabled={isBusy}
            >
              <Lock size={16} />
              <span>
                {busyAction === 'unlock'
                  ? t('settings.sync.unlocking', { defaultValue: 'Unlocking...' })
                  : t('settings.sync.unlock_action', { defaultValue: 'Unlock' })}
              </span>
            </button>
          </div>
        </SettingsSection>
      ) : (
        /* Connected / Active Mode */
        <>
          {/* Section 2: Vault Overview */}
          <SettingsSection
            title={t('settings.sync.overview_title', { defaultValue: 'Sync Vault' })}
            description={t('settings.sync.overview_description', {
              defaultValue: 'End-to-end encrypted incremental sync. Audio recordings remain local.',
            })}
            icon={<ShieldCheck size={20} />}
          >
            {/* Status Panel Row */}
            <div className="sync-status-panel">
              <div className="sync-status-header-row">
                <div className="sync-status-heading">
                  <div className={`sync-status-icon is-${status.state}`}>
                    {status.state === 'syncing' ? (
                      <RefreshCw size={20} className="spin" />
                    ) : status.state === 'paused' ? (
                      <Pause size={20} />
                    ) : (
                      <ShieldCheck size={20} />
                    )}
                  </div>
                  <div>
                    <div className="sync-status-title-group">
                      <strong>{t(`settings.sync.status_${status.state}`, { defaultValue: status.state })}</strong>
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
                    {status.vaultId && (
                      <span className="sync-vault-id-label">
                        {t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}: <code>{status.vaultId.slice(0, 16)}...</code>
                      </span>
                    )}
                  </div>
                </div>

                <div className="sync-status-actions">
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => setShowPairingModal(true)}
                    disabled={isBusy}
                  >
                    <QrCode size={15} />
                    <span>{t('settings.sync.pair_device_action', { defaultValue: 'Pair new device' })}</span>
                  </button>
                </div>
              </div>

              {/* 4 Metric Cards */}
              <div className="sync-metric-grid">
                <div className="sync-metric-card">
                  <span>{t('settings.sync.last_success', { defaultValue: 'Last success' })}</span>
                  <strong>{formatDate(status.lastSuccessAtMs, t('settings.sync.never', { defaultValue: 'Never' }), i18n.language)}</strong>
                </div>
                <div className="sync-metric-card">
                  <span>{t('settings.sync.pending', { defaultValue: 'Pending upload' })}</span>
                  <strong>{status.pendingOperationCount}</strong>
                </div>
                <div className="sync-metric-card">
                  <span>{t('settings.sync.preset', { defaultValue: 'Sync preset' })}</span>
                  <strong>{t(`settings.sync.preset_${status.preset}`, { defaultValue: status.preset ?? '-' })}</strong>
                </div>
                <div className="sync-metric-card">
                  <span>{t('settings.sync.conflicts', { defaultValue: 'Conflicts' })}</span>
                  <strong className={status.conflictCount > 0 ? 'sync-text-warning' : ''}>
                    {status.conflictCount}
                  </strong>
                </div>
              </div>

              {/* Error banner if last error */}
              {status.lastError && (
                <div className="sync-banner-box is-error">
                  <AlertCircle size={16} />
                  <span>{status.lastError.message || status.lastError.code}</span>
                </div>
              )}

              {/* Action buttons */}
              <div className="sync-status-actions">
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={handleRunNow}
                  disabled={isBusy || status.state === 'syncing' || status.state === 'paused'}
                >
                  <RefreshCw size={15} className={status.state === 'syncing' ? 'spin' : ''} />
                  <span>
                    {status.state === 'syncing'
                      ? t('settings.sync.syncing', { defaultValue: 'Syncing...' })
                      : t('settings.sync.run_now', { defaultValue: 'Sync now' })}
                  </span>
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void handleSetPaused(status.state !== 'paused')}
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
                  className="btn btn-secondary"
                  onClick={handleLock}
                  disabled={isBusy}
                >
                  <Lock size={15} />
                  <span>{t('settings.sync.lock_action', { defaultValue: 'Lock vault' })}</span>
                </button>
              </div>
            </div>

            {/* Scope Preset Selection */}
            <SettingsItem
              title={t('settings.sync.preset', { defaultValue: 'Sync preset' })}
              hint={t('settings.sync.preset_hint', {
                defaultValue: 'Choose which data domains participate in cloud sync.',
              })}
              layout="vertical"
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', width: '100%' }}>
                <div className="settings-scenario-cards three-columns" style={{ width: '100%', padding: 0, background: 'transparent' }}>
                  {[
                    {
                      id: 'content' as const,
                      label: t('settings.sync.preset_content', { defaultValue: 'Content' }),
                      description: t('settings.sync.scope_content_desc', { defaultValue: 'Transcripts & Projects' }),
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
                    const isSelected = selectedConnectedPreset === p.id;
                    return (
                      <button
                        key={p.id}
                        type="button"
                        className={`settings-scenario-card${isSelected ? ' active' : ''}`}
                        onClick={() => setPresetOverride(p.id)}
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
                {selectedConnectedPreset !== status.preset && (
                  <button
                    type="button"
                    className="btn btn-primary btn-sm"
                    style={{ alignSelf: 'flex-start' }}
                    onClick={() => void handleChangePreset(selectedConnectedPreset)}
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

          {/* Section 3: Security & Key Management */}
          <SettingsSection
            title={t('settings.sync.security_title', { defaultValue: 'Security & Credentials' })}
            icon={<KeyRound size={20} />}
          >
            {/* Change Master Password Accordion */}
            <SettingsAccordion
              title={(
                <div className="settings-accordion-copy">
                  <div className="settings-accordion-copy-title"><KeyRound size={16} />{t('settings.sync.change_password', { defaultValue: 'Change master password' })}</div>
                  <div className="settings-accordion-copy-hint">{t('settings.sync.change_password_hint', { defaultValue: 'Update the encryption master password for this vault.' })}</div>
                </div>
              )}
            >
              <SettingsItem indent={true} title={t('settings.sync.current_password', { defaultValue: 'Current password' })}>
                <input
                  className="settings-input"
                  type="password"
                  autoComplete="current-password"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  disabled={isBusy}
                  style={{ width: '260px' }}
                />
              </SettingsItem>

              <SettingsItem indent={true} title={t('settings.sync.next_password', { defaultValue: 'New master password' })}>
                <input
                  className="settings-input"
                  type="password"
                  autoComplete="new-password"
                  value={nextPassword}
                  onChange={(e) => setNextPassword(e.target.value)}
                  disabled={isBusy}
                  style={{ width: '260px' }}
                />
              </SettingsItem>

              <SettingsItem indent={true} title={t('settings.sync.confirm_next_password', { defaultValue: 'Confirm new master password' })}>
                <input
                  className="settings-input"
                  type="password"
                  autoComplete="new-password"
                  value={confirmNextPassword}
                  onChange={(e) => setConfirmNextPassword(e.target.value)}
                  disabled={isBusy}
                  style={{ width: '260px' }}
                />
              </SettingsItem>

              {passwordError && (
                <div className="sync-banner-row">
                  <div className="sync-banner-box is-error">
                    <AlertCircle size={16} />
                    <span>{passwordError}</span>
                  </div>
                </div>
              )}

              <div className="sync-wizard-actions">
                <button
                  type="button"
                  className="btn btn-primary btn-sm"
                  onClick={handleChangeMasterPassword}
                  disabled={isBusy || !currentPassword || !nextPassword || !confirmNextPassword}
                >
                  {busyAction === 'change_master_password'
                    ? t('settings.sync.changing_password', { defaultValue: 'Updating password...' })
                    : t('settings.sync.change_password_action', { defaultValue: 'Update password' })}
                </button>
              </div>
            </SettingsAccordion>

            {/* Recovery Key Accordion */}
            <SettingsAccordion
              title={(
                <div className="settings-accordion-copy">
                  <div className="settings-accordion-copy-title"><ShieldCheck size={16} />{t('settings.sync.recovery_key_manage_title', { defaultValue: 'Emergency Recovery Key' })}</div>
                  <div className="settings-accordion-copy-hint">{t('settings.sync.recovery_key_manage_hint', { defaultValue: 'Used to recover your data if you forget the master password.' })}</div>
                </div>
              )}
            >
              <div className="sync-recovery-wrapper">
                <div className="sync-recovery-top-row">
                  <div>
                    <strong>{t('settings.sync.recovery_key_manage_title', { defaultValue: 'Emergency Recovery Key' })}</strong>
                    <span>{t('settings.sync.recovery_key_manage_hint', { defaultValue: 'Used to recover your data if you forget the master password.' })}</span>
                  </div>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={handleGenerateRecoveryKey}
                    disabled={isBusy}
                  >
                    <Sparkles size={15} />
                    <span>{t('settings.sync.regenerate_key_action', { defaultValue: 'Generate new key' })}</span>
                  </button>
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
                          onClick={async () => {
                            if (navigator.clipboard?.writeText) {
                              await navigator.clipboard.writeText(recoveryKey);
                              setCopiedKey(true);
                              setTimeout(() => setCopiedKey(false), 2000);
                            }
                          }}
                        >
                          {copiedKey ? <Check size={14} /> : <Copy size={14} />}
                          <span>{copiedKey ? t('common.copied', { defaultValue: 'Copied' }) : t('common.copy', { defaultValue: 'Copy' })}</span>
                        </button>
                        <button
                          type="button"
                          className="btn btn-secondary btn-sm"
                          onClick={handleExportRecoveryKey}
                          disabled={isBusy}
                        >
                          <Download size={14} />
                          <span>{t('settings.sync.export_recovery_key', { defaultValue: 'Export recovery key' })}</span>
                        </button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </SettingsAccordion>

            {/* Disconnect action */}
            <SettingsItem
              title={t('settings.sync.disconnect_title', { defaultValue: 'Disconnect this device' })}
              hint={t('settings.sync.disconnect_hint', { defaultValue: 'Local data stays intact. The remote vault is not deleted.' })}
            >
              <button
                type="button"
                className="btn btn-danger btn-sm"
                onClick={() => void handleDisconnect()}
                disabled={isBusy}
              >
                <Unplug size={15} />
                <span>{t('settings.sync.disconnect_action', { defaultValue: 'Disconnect' })}</span>
              </button>
            </SettingsItem>
          </SettingsSection>

          {/* Section 4: Conflict Center */}
          <SyncConflictCenter
            conflictCount={status.conflictCount}
            disabled={false}
          />
        </>
      )}

      {/* Pairing Modal */}
      <Modal
        isOpen={showPairingModal}
        onClose={() => setShowPairingModal(false)}
        title={t('settings.sync.pair_device_modal_title', { defaultValue: 'Pair a New Device' })}
        size="sm"
        footer={(
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => setShowPairingModal(false)}
          >
            {t('settings.sync.close', { defaultValue: 'Close' })}
          </button>
        )}
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
          <p className="sync-pairing-instructions">
            {t('settings.sync.pairing_instructions', {
              defaultValue: 'Copy this pairing code and paste it on your second device (Sona -> Cloud Sync -> Join vault).',
            })}
          </p>
          <div className="sync-pairing-token-box">
            <textarea
              readOnly
              className="settings-input sync-token-textarea"
              value={pairingToken}
              onClick={(e) => (e.target as HTMLTextAreaElement).select()}
              style={{ height: '90px' }}
            />
            <button
              type="button"
              className="btn btn-primary btn-sm"
              style={{ alignSelf: 'flex-start' }}
              onClick={async () => {
                if (navigator.clipboard?.writeText) {
                  await navigator.clipboard.writeText(pairingToken);
                  setCopiedToken(true);
                  setTimeout(() => setCopiedToken(false), 2000);
                }
              }}
            >
              {copiedToken ? <Check size={14} /> : <Copy size={14} />}
              <span>{copiedToken ? t('common.copied', { defaultValue: 'Copied' }) : t('settings.sync.copy_pairing_token', { defaultValue: 'Copy pairing code' })}</span>
            </button>
          </div>
          <div className="sync-pairing-note">
            <HelpCircle size={16} />
            <span>
              {t('settings.sync.pairing_security_note', {
                defaultValue: 'The pairing token only contains server connection metadata. You will still need to enter your Master Password on the second device.',
              })}
            </span>
          </div>
        </div>
      </Modal>
    </SettingsTabContainer>
  );
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

function formatDate(timestamp: number | null, fallback: string, locale?: string): string {
  if (!timestamp) return fallback;
  try {
    return new Intl.DateTimeFormat(locale || undefined, {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(timestamp));
  } catch {
    return fallback;
  }
}

export default SettingsSyncTab;
