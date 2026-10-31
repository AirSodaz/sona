import {
  AlertCircle,
  ArrowLeft,
  Check,
  CheckCircle2,
  Edit2,
  Globe,
  Key,
  Plus,
  RefreshCw,
  Server,
  Trash2,
  Tv,
  User,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { v4 as uuidv4 } from 'uuid';
import {
  deleteMediaServerProfile,
  loginMediaServer,
  saveMediaServerProfile,
  testMediaServerConnection,
} from '../../../../services/mediaServer/mediaServerService';
import type {
  MediaServerInfo,
  MediaServerProfile,
  MediaServerType,
} from '../../../../types/mediaServer';
import { Modal } from '../../../Modal';
import './MediaServerManageModal.css';

export interface MediaServerManageModalProps {
  isOpen: boolean;
  onClose: () => void;
  profiles: MediaServerProfile[];
  activeProfileId: string | null;
  onProfilesUpdated: () => void | Promise<void>;
  onSelectActiveProfile: (profileId: string) => void | Promise<void>;
}

export function MediaServerManageModal({
  isOpen,
  onClose,
  profiles,
  activeProfileId,
  onProfilesUpdated,
  onSelectActiveProfile,
}: MediaServerManageModalProps): React.JSX.Element {
  const { t } = useTranslation();

  const [isEditing, setIsEditing] = useState<boolean>(false);
  const [editingId, setEditingId] = useState<string | null>(null);

  // Form states
  const [serverType, setServerType] = useState<MediaServerType>('emby');
  const [serverUrl, setServerUrl] = useState<string>('');
  const [name, setName] = useState<string>('');
  const [authMethod, setAuthMethod] = useState<'login' | 'apikey'>('login');
  const [username, setUsername] = useState<string>('');
  const [password, setPassword] = useState<string>('');
  const [apiKey, setApiKey] = useState<string>('');
  const [adminUserId, setAdminUserId] = useState<string>('');
  const [allowInvalidCerts, setAllowInvalidCerts] = useState<boolean>(false);

  // Operation status states
  const [isTesting, setIsTesting] = useState<boolean>(false);
  const [isLoggingIn, setIsLoggingIn] = useState<boolean>(false);
  const [testResult, setTestResult] = useState<MediaServerInfo | null>(null);
  const [formError, setFormError] = useState<string | null>(null);

  // List connection test status map: profileId -> { status, message }
  const [serverStatusMap, setServerStatusMap] = useState<
    Record<string, { status: 'testing' | 'ok' | 'error'; message: string }>
  >({});

  const resetForm = useCallback(() => {
    setServerType('emby');
    setServerUrl('');
    setName('');
    setAuthMethod('login');
    setUsername('');
    setPassword('');
    setApiKey('');
    setAdminUserId('');
    setAllowInvalidCerts(false);
    setFormError(null);
    setTestResult(null);
  }, []);

  useEffect(() => {
    if (isOpen) {
      setIsEditing(false);
      setEditingId(null);
      setFormError(null);
      setTestResult(null);
      resetForm();
    }
  }, [isOpen, resetForm]);

  const startAddServer = () => {
    resetForm();
    setEditingId(null);
    setIsEditing(true);
  };

  const startEditServer = (profile: MediaServerProfile) => {
    setEditingId(profile.id);
    setServerType(profile.serverType);
    setServerUrl(profile.serverUrl);
    setName(profile.name);
    setApiKey(profile.apiKey);
    setUsername(profile.username || '');
    setAdminUserId(profile.adminUserId || '');
    setAllowInvalidCerts(profile.allowInvalidCerts ?? false);
    setAuthMethod(profile.username ? 'login' : 'apikey');
    setPassword('');
    setFormError(null);
    setTestResult(null);
    setIsEditing(true);
  };

  const handleTestInList = async (profile: MediaServerProfile) => {
    setServerStatusMap((prev) => ({
      ...prev,
      [profile.id]: {
        status: 'testing',
        message: t('media_server.testing', { defaultValue: 'Testing...' }),
      },
    }));

    try {
      const info = await testMediaServerConnection(profile);
      setServerStatusMap((prev) => ({
        ...prev,
        [profile.id]: {
          status: 'ok',
          message: `v${info.version}`,
        },
      }));
    } catch (err: unknown) {
      setServerStatusMap((prev) => ({
        ...prev,
        [profile.id]: {
          status: 'error',
          message: err instanceof Error ? err.message : String(err),
        },
      }));
    }
  };

  const handleDelete = async (profileId: string) => {
    if (
      window.confirm(
        t('media_server.confirm_delete', { defaultValue: 'Delete this media server profile?' })
      )
    ) {
      try {
        await deleteMediaServerProfile(profileId);
        await onProfilesUpdated();
      } catch (err: unknown) {
        alert(err instanceof Error ? err.message : String(err));
      }
    }
  };

  const handleLoginAndFetchToken = async () => {
    if (!serverUrl.trim() || !username.trim()) {
      setFormError(
        t('media_server.error_url_user_required', {
          defaultValue: 'Server URL and username are required',
        })
      );
      return;
    }

    setFormError(null);
    setIsLoggingIn(true);

    try {
      const loginPayload = {
        serverUrl: serverUrl.trim(),
        serverType,
        username: username.trim(),
        password,
        allowInvalidCerts,
      };

      const result = await loginMediaServer(loginPayload);
      setApiKey(result.accessToken);
      if (result.userId) {
        setAdminUserId(result.userId);
      }
      try {
        const info = await testMediaServerConnection({
          id: editingId || 'temp',
          name: name.trim() || 'Temp',
          serverType,
          serverUrl: serverUrl.trim(),
          apiKey: result.accessToken,
          username: result.userName || username.trim(),
          adminUserId: result.userId,
          allowInvalidCerts,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        });
        setTestResult(info);
        if (!name.trim() && info.serverName) {
          setName(info.serverName);
        }
      } catch {
        // Token acquired successfully; info fetch was optional
      }
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoggingIn(false);
    }
  };

  const handleTestFormConnection = async () => {
    if (!serverUrl.trim() || !apiKey.trim()) {
      setFormError(
        t('media_server.error_url_key_required', {
          defaultValue: 'Server URL and API Key / Token are required to test',
        })
      );
      return;
    }

    setFormError(null);
    setIsTesting(true);
    setTestResult(null);

    try {
      const tempProfile: MediaServerProfile = {
        id: editingId || 'temp',
        name: name.trim() || 'Temp',
        serverType,
        serverUrl: serverUrl.trim(),
        apiKey: apiKey.trim(),
        username: username.trim() || undefined,
        adminUserId: adminUserId.trim() || undefined,
        allowInvalidCerts,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };

      const info = await testMediaServerConnection(tempProfile);
      setTestResult(info);
      if (!name.trim()) {
        setName(info.serverName || `${serverType === 'jellyfin' ? 'Jellyfin' : 'Emby'} Server`);
      }
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsTesting(false);
    }
  };

  const handleSaveProfile = async () => {
    if (!serverUrl.trim() || !apiKey.trim()) {
      setFormError(
        t('media_server.error_url_key_required', {
          defaultValue: 'Server URL and API Key / Token are required',
        })
      );
      return;
    }

    const profileToSave: MediaServerProfile = {
      id: editingId || uuidv4(),
      name:
        name.trim() ||
        (testResult?.serverName ?? `${serverType === 'jellyfin' ? 'Jellyfin' : 'Emby'} Server`),
      serverType,
      serverUrl: serverUrl.trim().replace(/\/+$/, ''),
      apiKey: apiKey.trim(),
      username: username.trim() || undefined,
      adminUserId: adminUserId.trim() || undefined,
      allowInvalidCerts,
      createdAt:
        editingId && profiles.find((p) => p.id === editingId)
          ? (profiles.find((p) => p.id === editingId)?.createdAt ?? Date.now())
          : Date.now(),
      updatedAt: Date.now(),
    };

    try {
      await saveMediaServerProfile(profileToSave);
      await onProfilesUpdated();
      setIsEditing(false);
      setEditingId(null);
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : String(err));
    }
  };

  const modalTitle = isEditing ? (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
      <button
        type="button"
        className="btn btn-xs btn-secondary"
        onClick={() => {
          setIsEditing(false);
          setEditingId(null);
        }}
      >
        <ArrowLeft size={13} style={{ marginRight: 4 }} />
        <span>{t('common.back', { defaultValue: 'Back' })}</span>
      </button>
      <span>
        {editingId
          ? t('media_server.edit_server', { defaultValue: 'Edit Media Server' })
          : t('media_server.add_server', { defaultValue: 'Add Media Server' })}
      </span>
    </div>
  ) : (
    t('media_server.manage_title', { defaultValue: 'Manage Media Servers' })
  );

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={modalTitle}
      size="lg"
      footer={
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'flex-end',
            gap: 8,
            width: '100%',
          }}
        >
          {isEditing ? (
            <>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => {
                  setIsEditing(false);
                  setEditingId(null);
                }}
              >
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                disabled={isTesting || !serverUrl.trim() || !apiKey.trim()}
                onClick={handleTestFormConnection}
              >
                {isTesting ? (
                  <>
                    <RefreshCw size={14} className="spin" style={{ marginRight: 6 }} />
                    <span>{t('media_server.testing', { defaultValue: 'Testing...' })}</span>
                  </>
                ) : (
                  <span>
                    {t('media_server.test_connection', { defaultValue: 'Test Connection' })}
                  </span>
                )}
              </button>
              <button type="button" className="btn btn-primary" onClick={handleSaveProfile}>
                {t('common.save', { defaultValue: 'Save Server' })}
              </button>
            </>
          ) : (
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              {t('common.close', { defaultValue: 'Close' })}
            </button>
          )}
        </div>
      }
    >
      <div className="media-server-manage-container">
        {!isEditing ? (
          <>
            <div className="media-server-list-header">
              <p className="media-server-list-desc">
                {t('media_server.manage_desc', {
                  defaultValue:
                    'Connect and manage multiple Emby and Jellyfin servers. Switch servers easily or queue jobs across different instances.',
                })}
              </p>
              <button type="button" className="btn btn-sm btn-primary" onClick={startAddServer}>
                <Plus size={14} style={{ marginRight: 4 }} />
                <span>{t('media_server.add_server', { defaultValue: 'Add Server' })}</span>
              </button>
            </div>

            {profiles.length === 0 ? (
              <div className="shared-empty-container">
                <div className="shared-empty-icon-wrap">
                  <Server size={22} />
                </div>
                <div className="shared-empty-info">
                  <h4 className="shared-empty-title">
                    {t('media_server.no_servers_configured', {
                      defaultValue: 'No media servers configured.',
                    })}
                  </h4>
                  <p className="media-server-field-desc">
                    {t('media_server.welcome_desc', {
                      defaultValue:
                        'Connect to your Emby or Jellyfin media server to browse libraries, select movies or TV series, transcribe audio offline with Sona, and sync subtitles directly back.',
                    })}
                  </p>
                </div>
                <button type="button" className="btn btn-sm btn-primary" onClick={startAddServer}>
                  <Plus size={14} style={{ marginRight: 4 }} />
                  <span>{t('media_server.add_server', { defaultValue: 'Add Server' })}</span>
                </button>
              </div>
            ) : (
              <div className="media-server-cards-list">
                {profiles.map((profile) => {
                  const isActive = profile.id === activeProfileId;
                  const status = serverStatusMap[profile.id];

                  return (
                    <div
                      key={profile.id}
                      className={`media-server-card ${isActive ? 'is-active' : ''}`}
                    >
                      <div className="media-server-card-main">
                        <div className="media-server-card-icon">
                          <Tv size={18} />
                        </div>

                        <div className="media-server-card-info">
                          <div className="media-server-card-header">
                            <span className="media-server-card-name">{profile.name}</span>
                            <span className="media-server-type-badge">{profile.serverType}</span>
                            {isActive && (
                              <span className="media-server-active-pill">
                                <Check size={11} />
                                <span>{t('media_server.active', { defaultValue: 'Active' })}</span>
                              </span>
                            )}
                          </div>

                          <div className="media-server-card-meta">
                            <span className="media-server-meta-item" title={profile.serverUrl}>
                              <Globe size={11} style={{ opacity: 0.7 }} />
                              <span>{profile.serverUrl}</span>
                            </span>
                            {profile.username && (
                              <span className="media-server-meta-item">
                                <User size={11} style={{ opacity: 0.7 }} />
                                <span>{profile.username}</span>
                              </span>
                            )}
                          </div>

                          {status && (
                            <div
                              className={`media-server-card-status ${
                                status.status === 'testing'
                                  ? 'is-testing'
                                  : status.status === 'ok'
                                    ? 'is-ok'
                                    : 'is-error'
                              }`}
                            >
                              {status.status === 'testing' ? (
                                <>
                                  <RefreshCw size={11} className="spin" />
                                  <span>
                                    {t('media_server.testing', { defaultValue: 'Testing...' })}
                                  </span>
                                </>
                              ) : status.status === 'ok' ? (
                                <>
                                  <span className="media-server-status-dot" />
                                  <span>
                                    {t('media_server.connected', { defaultValue: 'Connected' })}{' '}
                                    {status.message}
                                  </span>
                                </>
                              ) : (
                                <>
                                  <span className="media-server-status-dot" />
                                  <span title={status.message}>
                                    {t('common.error', { defaultValue: 'Error' })}: {status.message}
                                  </span>
                                </>
                              )}
                            </div>
                          )}
                        </div>
                      </div>

                      <div className="media-server-card-actions">
                        {!isActive && (
                          <button
                            type="button"
                            className="btn btn-xs btn-secondary"
                            onClick={() => void onSelectActiveProfile(profile.id)}
                            aria-label={t('media_server.set_active', { defaultValue: 'Select' })}
                            data-tooltip={t('media_server.set_active', { defaultValue: 'Select' })}
                            data-tooltip-pos="top"
                          >
                            {t('media_server.set_active', { defaultValue: 'Select' })}
                          </button>
                        )}
                        <button
                          type="button"
                          className={`model-action-icon model-action-test${status?.status ? ` ${status.status}` : ''}`}
                          onClick={() => void handleTestInList(profile)}
                          disabled={status?.status === 'testing'}
                          aria-label={`${t('media_server.test', { defaultValue: 'Test' })} ${profile.name}`}
                          data-tooltip={
                            status?.status === 'testing'
                              ? t('media_server.testing', { defaultValue: 'Testing...' })
                              : status?.status === 'ok'
                                ? `✓ ${t('media_server.connected', { defaultValue: 'Connected' })} ${status.message}`
                                : status?.status === 'error'
                                  ? `✕ ${status.message || t('common.error', { defaultValue: 'Error' })}`
                                  : t('media_server.test', { defaultValue: 'Test Connection' })
                          }
                          data-tooltip-pos="top"
                        >
                          <RefreshCw
                            size={14}
                            className={status?.status === 'testing' ? 'spin' : ''}
                          />
                        </button>
                        <button
                          type="button"
                          className="model-action-icon model-action-edit"
                          onClick={() => startEditServer(profile)}
                          aria-label={`${t('common.edit', { defaultValue: 'Edit' })} ${profile.name}`}
                          data-tooltip={t('common.edit', { defaultValue: 'Edit' })}
                          data-tooltip-pos="top"
                        >
                          <Edit2 size={14} />
                        </button>
                        <button
                          type="button"
                          className="model-action-icon model-action-delete"
                          onClick={() => void handleDelete(profile.id)}
                          aria-label={`${t('common.delete', { defaultValue: 'Delete' })} ${profile.name}`}
                          data-tooltip={t('common.delete', { defaultValue: 'Delete' })}
                          data-tooltip-pos="top"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </>
        ) : (
          <div className="media-server-form-body">
            {formError && (
              <div className="emby-alert-box emby-alert-error" role="alert">
                <AlertCircle size={15} style={{ flexShrink: 0 }} />
                <span>{formError}</span>
              </div>
            )}

            {testResult && (
              <div className="emby-alert-box emby-alert-success">
                <CheckCircle2 size={15} style={{ flexShrink: 0 }} />
                <span>
                  {t('media_server.connection_successful', {
                    defaultValue: 'Connected successfully to',
                  })}{' '}
                  <strong>{testResult.serverName}</strong> (v{testResult.version})
                  {testResult.serverType === 'jellyfin' && (
                    <span style={{ display: 'block', marginTop: 2, fontSize: 11, opacity: 0.9 }}>
                      {t('media_server.jellyfin_native_active', {
                        defaultValue:
                          'Native Jellyfin Subtitle Integration active (No plugin required)',
                      })}
                    </span>
                  )}
                </span>
              </div>
            )}

            {/* Server Type Selection */}
            <div className="emby-form-field">
              <span className="emby-form-label">
                {t('media_server.server_type', { defaultValue: 'Server Type' })}
              </span>
              <div className="media-server-segmented-group">
                <button
                  type="button"
                  className={`media-server-segmented-btn ${serverType === 'emby' ? 'active' : ''}`}
                  onClick={() => setServerType('emby')}
                >
                  <Tv size={14} />
                  <span>Emby</span>
                  <span className="media-server-field-desc" style={{ marginLeft: 4, opacity: 0.7 }}>
                    Emby Media Server
                  </span>
                </button>
                <button
                  type="button"
                  className={`media-server-segmented-btn ${serverType === 'jellyfin' ? 'active' : ''}`}
                  onClick={() => setServerType('jellyfin')}
                >
                  <Tv size={14} />
                  <span>Jellyfin</span>
                  <span className="media-server-field-desc" style={{ marginLeft: 4, opacity: 0.7 }}>
                    Jellyfin Media Server
                  </span>
                </button>
              </div>
            </div>

            {/* Server URL */}
            <div className="emby-form-field">
              <span className="emby-form-label">
                {t('media_server.server_url', { defaultValue: 'Server URL' })}
              </span>
              <input
                id="server-url-input"
                type="text"
                className="emby-input"
                placeholder={
                  serverType === 'jellyfin'
                    ? 'http://192.168.1.100:8096'
                    : 'http://192.168.1.100:8096'
                }
                value={serverUrl}
                onChange={(e) => setServerUrl(e.target.value)}
              />
              <span className="media-server-field-desc">
                {t('emby.modal_url_label', {
                  defaultValue: 'Server URL (including port and path)',
                })}
              </span>
            </div>

            {/* SSL Certificate check */}
            <label className="emby-checkbox-field">
              <input
                type="checkbox"
                checked={allowInvalidCerts}
                onChange={(e) => setAllowInvalidCerts(e.target.checked)}
              />
              <span>
                {t('media_server.allow_invalid_certs', {
                  defaultValue: 'Allow invalid / self-signed SSL certificates (HTTPS)',
                })}
              </span>
            </label>

            {/* Auth Method Segmented */}
            <div className="emby-form-field">
              <span className="emby-form-label">
                {t('media_server.auth_method', { defaultValue: 'Authentication Method' })}
              </span>
              <div className="media-server-segmented-group">
                <button
                  type="button"
                  className={`media-server-segmented-btn ${authMethod === 'login' ? 'active' : ''}`}
                  onClick={() => setAuthMethod('login')}
                >
                  <User size={14} />
                  <span>{t('media_server.account_login', { defaultValue: 'Account Login' })}</span>
                </button>
                <button
                  type="button"
                  className={`media-server-segmented-btn ${authMethod === 'apikey' ? 'active' : ''}`}
                  onClick={() => setAuthMethod('apikey')}
                >
                  <Key size={14} />
                  <span>
                    {t('media_server.manual_api_key', { defaultValue: 'API Key / Token' })}
                  </span>
                </button>
              </div>
            </div>

            {/* Login fields or API Key field */}
            {authMethod === 'login' ? (
              <div className="media-server-login-card">
                <div className="media-server-form-row">
                  <div className="emby-form-field" style={{ flex: 1 }}>
                    <span className="emby-form-label">
                      {t('media_server.username', { defaultValue: 'Username' })}
                    </span>
                    <input
                      id="username-input"
                      type="text"
                      className="emby-input"
                      placeholder={t('emby.modal_username_placeholder', {
                        defaultValue: 'e.g. admin',
                      })}
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      autoComplete="username"
                    />
                  </div>
                  <div className="emby-form-field" style={{ flex: 1 }}>
                    <span className="emby-form-label">
                      {t('media_server.password', { defaultValue: 'Password' })}
                    </span>
                    <input
                      id="password-input"
                      type="password"
                      className="emby-input"
                      placeholder={t('emby.modal_password_placeholder', {
                        defaultValue: 'Password (leave blank if none)',
                      })}
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      autoComplete="current-password"
                    />
                  </div>
                </div>

                <div className="media-server-token-row">
                  <div className="media-server-token-status">
                    {apiKey ? (
                      <span className="media-server-token-ready">
                        <CheckCircle2 size={14} />
                        <span>
                          {t('emby.token_ready', { defaultValue: 'Token acquired and ready' })}
                        </span>
                      </span>
                    ) : (
                      <span className="media-server-token-desc">
                        {t('media_server.login_tip', {
                          defaultValue: 'Logs in once to obtain and store an API access token.',
                        })}
                      </span>
                    )}
                  </div>
                  <button
                    type="button"
                    className="btn btn-sm btn-secondary"
                    disabled={isLoggingIn || !serverUrl.trim() || !username.trim()}
                    onClick={handleLoginAndFetchToken}
                  >
                    {isLoggingIn ? (
                      <>
                        <RefreshCw size={13} className="spin" style={{ marginRight: 5 }} />
                        <span>
                          {t('media_server.logging_in', { defaultValue: 'Logging in...' })}
                        </span>
                      </>
                    ) : (
                      <span>
                        {t('media_server.login_and_fetch_token', {
                          defaultValue: 'Login & Obtain Token',
                        })}
                      </span>
                    )}
                  </button>
                </div>
              </div>
            ) : (
              <div className="emby-form-field">
                <span className="emby-form-label">
                  {t('media_server.api_key_or_token', { defaultValue: 'API Key / Access Token' })}
                </span>
                <input
                  id="api-key-input"
                  type="password"
                  className="emby-input"
                  placeholder="e.g. 5a1b2c3d4e..."
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
              </div>
            )}

            {/* Display Name */}
            <div className="emby-form-field">
              <span className="emby-form-label">
                {t('media_server.profile_name', { defaultValue: 'Display Name' })}
              </span>
              <input
                id="profile-name-input"
                type="text"
                className="emby-input"
                placeholder={name || `${serverType === 'jellyfin' ? 'Jellyfin' : 'Emby'} Server`}
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
}
