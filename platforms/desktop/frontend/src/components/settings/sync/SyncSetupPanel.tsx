import React from 'react';
import {
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCircle2,
  DatabaseZap,
  HelpCircle,
  Layers,
  Link2,
  Server,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type {
  SyncCreateRequest,
  SyncCreateResult,
  SyncJoinPreview,
  SyncPresetV1,
  SyncPreviewJoinRequest,
  SyncProviderDescriptor,
  SyncRunResult,
  WebDavObjectStoreConfig,
} from '../../../types/sync';
import {
  SYNC_PROVIDER_PRESETS,
  type WellKnownSyncProviderId,
  detectProviderPresetId,
} from './SyncProviderPresets';
import { decodeSyncPairingToken } from './syncPairing';
import { SettingsItem } from '../SettingsLayout';
import { Switch } from '../../Switch';

type SetupMode = 'create' | 'join';

interface SyncSetupPanelProps {
  busyAction: string | null;
  onCreate: (request: SyncCreateRequest) => Promise<SyncCreateResult>;
  onJoin: (request: SyncPreviewJoinRequest) => Promise<SyncRunResult>;
  onPreviewJoin: (request: SyncPreviewJoinRequest) => Promise<SyncJoinPreview>;
  onTestProvider: (config: WebDavObjectStoreConfig) => Promise<SyncProviderDescriptor>;
}

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
  const [createStep, setCreateStep] = React.useState<1 | 2>(1);

  // Form states
  const [selectedPresetId, setSelectedPresetId] = React.useState<WellKnownSyncProviderId>('nutstore');
  const [provider, setProvider] = React.useState<WebDavObjectStoreConfig>({
    serverUrl: SYNC_PROVIDER_PRESETS[0].defaultServerUrl,
    remoteRoot: 'Sona',
    username: '',
    password: '',
  });
  const [vaultId, setVaultId] = React.useState('');
  const [masterPassword, setMasterPassword] = React.useState('');
  const [confirmPassword, setConfirmPassword] = React.useState('');
  const [preset, setPreset] = React.useState<SyncPresetV1>('standard');
  const [createRecoveryKey, setCreateRecoveryKey] = React.useState(true);

  // Pairing token state
  const [pairingTokenInput, setPairingTokenInput] = React.useState('');
  const [parsedTokenInfo, setParsedTokenInfo] = React.useState<string | null>(null);

  // Status & preview
  const [testSuccess, setTestSuccess] = React.useState<string | null>(null);
  const [preview, setPreview] = React.useState<SyncJoinPreview | null>(null);
  const [validationError, setValidationError] = React.useState<string | null>(null);
  const isBusy = busyAction !== null;

  const handleSelectPreset = (presetId: WellKnownSyncProviderId) => {
    setSelectedPresetId(presetId);
    const found = SYNC_PROVIDER_PRESETS.find((p) => p.id === presetId);
    if (found) {
      setProvider((current) => ({
        ...current,
        serverUrl: found.defaultServerUrl || current.serverUrl,
        remoteRoot: found.defaultRemoteRoot || 'Sona',
      }));
    }
    setTestSuccess(null);
    setValidationError(null);
  };

  const updateProvider = (patch: Partial<WebDavObjectStoreConfig>) => {
    setProvider((current) => {
      const next = { ...current, ...patch };
      if (patch.serverUrl !== undefined) {
        setSelectedPresetId(detectProviderPresetId(patch.serverUrl));
      }
      return next;
    });
    setTestSuccess(null);
    setPreview(null);
    setValidationError(null);
  };

  // Handle pasting pairing token in Join mode
  const handleTokenChange = (text: string) => {
    setPairingTokenInput(text);
    setValidationError(null);
    const decoded = decodeSyncPairingToken(text);
    if (decoded) {
      setProvider((current) => ({
        ...current,
        serverUrl: decoded.serverUrl,
        remoteRoot: decoded.remoteRoot,
        username: decoded.username,
        password: decoded.providerPassword || current.password,
      }));
      setVaultId(decoded.vaultId);
      setParsedTokenInfo(`${decoded.username} @ ${decoded.serverUrl} (Vault: ${decoded.vaultId.slice(0, 8)}...)`);
    } else {
      setParsedTokenInfo(null);
      if (text.trim().length > 10) {
        setValidationError(t('settings.sync.invalid_token', { defaultValue: 'Invalid pairing token format.' }));
      }
    }
  };

  const runTestConnection = async () => {
    setValidationError(null);
    setTestSuccess(null);
    const error = providerError(provider);
    if (error === 'https') {
      setValidationError(t('settings.sync.validation_https', { defaultValue: 'WebDAV requires an HTTPS server URL.' }));
      return;
    }
    if (error) {
      setValidationError(t('settings.sync.validation_provider', { defaultValue: 'Complete the WebDAV server, username, and password fields.' }));
      return;
    }
    try {
      const descriptor = await onTestProvider(provider);
      setTestSuccess(descriptor.displayName);
    } catch {
      // Handled by parent error toast
    }
  };

  const validateStep1 = (): boolean => {
    const error = providerError(provider);
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

  const validateCreate = (): boolean => {
    if (!validateStep1()) return false;
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

  const handleNextStep = () => {
    if (validateStep1()) {
      setCreateStep(2);
    }
  };

  const handleCreateSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!validateCreate()) return;
    try {
      await onCreate({
        provider,
        preset,
        masterPassword,
        createRecoveryKey,
      });
    } catch {
      // Structured error handled by parent
    }
  };

  const handleJoinSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!validateJoin()) return;
    const request: SyncPreviewJoinRequest = {
      provider,
      vaultId: vaultId.trim(),
      masterPassword,
    };
    try {
      setPreview(await onPreviewJoin(request));
    } catch {
      // Handled by parent
    }
  };

  const currentPresetMeta = SYNC_PROVIDER_PRESETS.find((p) => p.id === selectedPresetId);

  return (
    <div className="sync-setup-panel">
      {/* Top Mode Selection - Unified with Models & LLM Tabs */}
      <div
        className="settings-scenario-cards"
        role="tablist"
        aria-label={t('settings.sync.setup_mode', { defaultValue: 'Sync setup mode' })}
      >
        {([
          {
            value: 'create' as const,
            icon: <DatabaseZap size={18} />,
            label: t('settings.sync.create_tab', { defaultValue: 'Create vault' }),
            description: t('settings.sync.create_tab_desc', {
              defaultValue: 'Initialize a new end-to-end encrypted sync vault on your WebDAV server',
            }),
          },
          {
            value: 'join' as const,
            icon: <Link2 size={18} />,
            label: t('settings.sync.join_tab', { defaultValue: 'Join vault' }),
            description: t('settings.sync.join_tab_desc', {
              defaultValue: 'Connect and sync with an existing vault using a pairing code or server credentials',
            }),
          },
        ]).map((option) => (
          <button
            key={option.value}
            type="button"
            role="tab"
            aria-selected={mode === option.value}
            aria-label={option.label}
            className={`settings-scenario-card${mode === option.value ? ' active' : ''}`}
            onClick={() => {
              setMode(option.value);
              setTestSuccess(null);
              setValidationError(null);
            }}
          >
            <span className="settings-scenario-card-icon">{option.icon}</span>
            <span className="settings-scenario-card-text">
              <span className="settings-scenario-card-label">{option.label}</span>
              <span className="settings-scenario-card-description">{option.description}</span>
            </span>
          </button>
        ))}
      </div>

      {validationError && (
        <div className="sync-inline-error" role="alert">
          <span>{validationError}</span>
        </div>
      )}

      {/* Mode A: Create Vault Wizard */}
      {mode === 'create' ? (
        <form className="sync-form" onSubmit={handleCreateSubmit}>
          {/* Wizard Step Indicator */}
          <div className="sync-step-indicator">
            <div className={`sync-step-node ${createStep === 1 ? 'is-active' : ''}`}>
              <span className="sync-step-num">1</span>
              <span className="sync-step-text">{t('settings.sync.step1_title', { defaultValue: 'Storage Provider' })}</span>
            </div>
            <div className="sync-step-line" />
            <div className={`sync-step-node ${createStep === 2 ? 'is-active' : ''}`}>
              <span className="sync-step-num">2</span>
              <span className="sync-step-text">{t('settings.sync.step2_title', { defaultValue: 'Security & Scope' })}</span>
            </div>
          </div>

          {createStep === 1 ? (
            <>
              {/* Provider Selection */}
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

              {/* Provider Config Guidance */}
              {currentPresetMeta && (
                <div className="sync-preset-help-banner">
                  <HelpCircle size={16} />
                  <div>
                    <div>{t(currentPresetMeta.helpKey, { defaultValue: currentPresetMeta.helpDefault })}</div>
                    {currentPresetMeta.authDocUrl && (
                      <a
                        href={currentPresetMeta.authDocUrl}
                        target="_blank"
                        rel="noreferrer"
                        className="sync-help-link"
                      >
                        {t('settings.sync.view_auth_guide', { defaultValue: 'View setup guide' })}
                      </a>
                    )}
                  </div>
                </div>
              )}

              {/* Connection Form Fields */}
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
                  style={{ width: '320px' }}
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
                  style={{ width: '200px' }}
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
                  style={{ width: '260px' }}
                />
              </SettingsItem>

              <SettingsItem
                title={t('settings.sync.provider_password', { defaultValue: 'WebDAV password' })}
                hint={t('settings.sync.provider_password_hint', { defaultValue: 'Dedicated app password (recommended)' })}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
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
                    style={{ width: '200px' }}
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
                <div className="sync-test-success-chip">
                  <CheckCircle2 size={16} />
                  <span>{t('settings.sync.test_passed', { defaultValue: 'Connected successfully to {{name}}', name: testSuccess })}</span>
                </div>
              )}

              <div className="sync-wizard-actions">
                <div className="sync-actions-spacer" />
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={handleNextStep}
                  disabled={isBusy}
                >
                  <span>{t('settings.sync.next_step', { defaultValue: 'Next step: Security & Scope' })}</span>
                  <ArrowRight size={16} />
                </button>
              </div>
            </>
          ) : (
            <>
              {/* Step 2: Security & Encryption */}
              <div className="sync-security-callout">
                <ShieldCheck size={20} />
                <div>
                  <strong>{t('settings.sync.e2e_encryption_title', { defaultValue: 'End-to-End Encryption' })}</strong>
                  <p>{t('settings.sync.e2e_encryption_desc', {
                    defaultValue: 'Your data is encrypted on this device before uploading. Nobody, not even the storage provider, can decrypt your transcripts without the master password.',
                  })}</p>
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
                  style={{ width: '260px' }}
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
                  style={{ width: '260px' }}
                />
              </SettingsItem>

              {/* Sync Scope Selection Cards */}
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
                      items: [t('settings.sync.scope_transcripts', { defaultValue: 'Transcripts' }), t('settings.sync.scope_summaries', { defaultValue: 'Summaries' })],
                    },
                    {
                      id: 'standard' as const,
                      label: t('settings.sync.preset_standard', { defaultValue: 'Standard' }),
                      description: t('settings.sync.scope_standard_desc', { defaultValue: 'Recommended for daily sync' }),
                      badge: t('common.recommended', { defaultValue: 'Recommended' }),
                      items: [
                        t('settings.sync.scope_transcripts', { defaultValue: 'Transcripts' }),
                        t('settings.sync.scope_summaries', { defaultValue: 'Summaries' }),
                        t('settings.sync.scope_rules', { defaultValue: 'Vocabulary & Rules' }),
                        t('settings.sync.scope_templates', { defaultValue: 'Templates' }),
                      ],
                    },
                    {
                      id: 'full' as const,
                      label: t('settings.sync.preset_full', { defaultValue: 'Full workspace' }),
                      description: t('settings.sync.scope_full_desc', { defaultValue: 'All settings & profiles' }),
                      items: [
                        t('settings.sync.scope_transcripts', { defaultValue: 'Transcripts' }),
                        t('settings.sync.scope_speakers', { defaultValue: 'Speaker profiles' }),
                        t('settings.sync.scope_automation', { defaultValue: 'Automation rules' }),
                        t('settings.sync.scope_settings', { defaultValue: 'Workspace settings' }),
                      ],
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
                  checked={createRecoveryKey}
                  onChange={setCreateRecoveryKey}
                  disabled={isBusy}
                />
              </SettingsItem>

              <div className="sync-wizard-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setCreateStep(1)}
                  disabled={isBusy}
                >
                  <ArrowLeft size={16} />
                  <span>{t('common.back', { defaultValue: 'Back' })}</span>
                </button>
                <div className="sync-actions-spacer" />
                <button type="submit" className="btn btn-primary" disabled={isBusy}>
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
        </form>
      ) : (
        /* Mode B: Join Vault */
        <form className="sync-form" onSubmit={handleJoinSubmit}>
          {/* Quick Pairing Token Accordion / Input */}
          <SettingsItem
            title={t('settings.sync.pairing_token_label', { defaultValue: 'Pairing code (Optional)' })}
            hint={t('settings.sync.pairing_token_hint', { defaultValue: 'Paste the pairing code (sonasync://...) generated on your primary device to auto-fill connection parameters' })}
            layout="vertical"
          >
            <textarea
              id="sync-pairing-token-input"
              className="settings-input sync-monospace-input sync-token-textarea"
              aria-label={t('settings.sync.pairing_token_label', { defaultValue: 'Pairing code' })}
              placeholder="sonasync://v1?data=eyJ..."
              value={pairingTokenInput}
              onChange={(e) => handleTokenChange(e.target.value)}
              disabled={isBusy}
              style={{ width: '100%' }}
            />
          </SettingsItem>

          {parsedTokenInfo && (
            <div className="sync-token-parsed-badge">
              <Check size={16} />
              <span>{t('settings.sync.token_parsed_success', { defaultValue: 'Recognized: {{info}}', info: parsedTokenInfo })}</span>
            </div>
          )}

          {/* Provider Preset Selector */}
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
              style={{ width: '320px' }}
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
              style={{ width: '200px' }}
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
              style={{ width: '260px' }}
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
              style={{ width: '260px' }}
            />
          </SettingsItem>

          <SettingsItem
            title={t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}
            hint={t('settings.sync.vault_id_hint', { defaultValue: 'Find this in the sync settings of your primary device' })}
          >
            <input
              id="sync-vault-id"
              className="settings-input sync-monospace-input"
              type="text"
              aria-label={t('settings.sync.vault_id', { defaultValue: 'Vault ID' })}
              spellCheck={false}
              value={vaultId}
              onChange={(e) => {
                setVaultId(e.target.value);
                setPreview(null);
              }}
              disabled={isBusy}
              style={{ width: '260px' }}
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
              style={{ width: '260px' }}
            />
          </SettingsItem>

          {preview && (
            <div className="sync-preview-card">
              <div className="sync-preview-header">
                <Sparkles size={16} />
                <strong>{t('settings.sync.preview_title', { defaultValue: 'Join preview' })}</strong>
              </div>
              <div className="sync-preview-grid">
                <div>
                  <span>{t('settings.sync.remote_changes', { defaultValue: 'Remote operations' })}</span>
                  <strong>{preview.remoteOperationCount}</strong>
                </div>
                <div>
                  <span>{t('settings.sync.projected_conflicts', { defaultValue: 'Projected conflicts' })}</span>
                  <strong>{preview.projectedConflictCount}</strong>
                </div>
              </div>
            </div>
          )}

          <div className="sync-wizard-actions">
            <div className="sync-actions-spacer" />
            {preview ? (
              <button
                type="button"
                className="btn btn-primary"
                disabled={isBusy}
                onClick={async () => {
                  try {
                    await onJoin({
                      provider,
                      vaultId: vaultId.trim(),
                      masterPassword,
                    });
                  } catch {
                    // Handled by parent
                  }
                }}
              >
                <Check size={16} />
                <span>
                  {busyAction === 'join'
                    ? t('settings.sync.joining', { defaultValue: 'Joining...' })
                    : t('settings.sync.confirm_join', { defaultValue: 'Confirm join' })}
                </span>
              </button>
            ) : (
              <button type="submit" className="btn btn-primary" disabled={isBusy}>
                <Link2 size={16} />
                <span>
                  {busyAction === 'preview_join'
                    ? t('settings.sync.previewing', { defaultValue: 'Preparing preview...' })
                    : t('settings.sync.preview_join', { defaultValue: 'Preview join' })}
                </span>
              </button>
            )}
          </div>
        </form>
      )}
    </div>
  );
}

export default SyncSetupPanel;
