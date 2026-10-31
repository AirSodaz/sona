import {
  AlertCircle,
  CheckCircle2,
  Edit2,
  Plus,
  RefreshCw,
  Server,
  Trash2,
  Tv,
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

  const [isEditing, setIsEditing] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);

  // Form State
  const [serverType, setServerType] = useState<MediaServerType>('emby');
  const [name, setName] = useState('');
  const [serverUrl, setServerUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [adminUserId, setAdminUserId] = useState<string | undefined>(undefined);
  const [allowInvalidCerts, setAllowInvalidCerts] = useState(false);
  const [authMethod, setAuthMethod] = useState<'login' | 'apikey'>('login');

  // Status & Feedback
  const [isTesting, setIsTesting] = useState(false);
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<MediaServerInfo | null>(null);
  const [perServerStatus, setPerServerStatus] = useState<
    Record<string, { status: 'testing' | 'ok' | 'error'; message?: string }>
  >({});

  const resetForm = useCallback(() => {
    setServerType('emby');
    setName('');
    setServerUrl('');
    setApiKey('');
    setUsername('');
    setPassword('');
    setAdminUserId(undefined);
    setAllowInvalidCerts(false);
    setAuthMethod('login');
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
    setName(profile.name);
    setServerUrl(profile.serverUrl);
    setApiKey(profile.apiKey);
    setUsername(profile.username || '');
    setPassword('');
    setAdminUserId(profile.adminUserId);
    setAllowInvalidCerts(profile.allowInvalidCerts);
    setAuthMethod(profile.apiKey ? 'apikey' : 'login');
    setFormError(null);
    setTestResult(null);
    setIsEditing(true);
  };

  const handleTestInList = async (profile: MediaServerProfile) => {
    setPerServerStatus((prev) => ({
      ...prev,
      [profile.id]: { status: 'testing' },
    }));

    try {
      const info = await testMediaServerConnection(profile);
      setPerServerStatus((prev) => ({
        ...prev,
        [profile.id]: { status: 'ok', message: `v${info.version}` },
      }));
    } catch (err: unknown) {
      setPerServerStatus((prev) => ({
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
        if (editingId === profileId) {
          setIsEditing(false);
          setEditingId(null);
        }
      } catch (err: unknown) {
        alert(err instanceof Error ? err.message : String(err));
      }
    }
  };

  const handleLoginAction = async () => {
    const cleanUrl = serverUrl.trim().replace(/\/+$/, '');
    const cleanUser = username.trim();
    if (!cleanUrl || !cleanUser) {
      setFormError(
        t('media_server.error_url_user_required', {
          defaultValue: 'Server URL and username are required',
        })
      );
      return;
    }

    setIsLoggingIn(true);
    setFormError(null);

    try {
      const res = await loginMediaServer({
        serverUrl: cleanUrl,
        serverType,
        username: cleanUser,
        password: password || undefined,
        allowInvalidCerts,
      });

      setApiKey(res.accessToken);
      setUsername(res.userName);
      setAdminUserId(res.userId);

      // Auto-set profile name if empty
      if (!name.trim()) {
        setName(`${serverType === 'jellyfin' ? 'Jellyfin' : 'Emby'} (${cleanUser})`);
      }

      setAuthMethod('apikey'); // Switch to token view
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoggingIn(false);
    }
  };

  const handleTestFormConnection = async () => {
    const cleanUrl = serverUrl.trim().replace(/\/+$/, '');
    const cleanKey = apiKey.trim();
    if (!cleanUrl || !cleanKey) {
      setFormError(
        t('media_server.error_url_key_required', {
          defaultValue: 'Server URL and API Key / Token are required to test',
        })
      );
      return;
    }

    setIsTesting(true);
    setFormError(null);
    setTestResult(null);

    try {
      const tempProfile: MediaServerProfile = {
        id: editingId || 'temp-test',
        name: name.trim() || 'Test Server',
        serverType,
        serverUrl: cleanUrl,
        apiKey: cleanKey,
        username: username.trim() || undefined,
        adminUserId,
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
    const cleanUrl = serverUrl.trim().replace(/\/+$/, '');
    const cleanKey = apiKey.trim();
    if (!cleanUrl || !cleanKey) {
      setFormError(
        t('media_server.error_url_key_required', {
          defaultValue: 'Server URL and API Key / Token are required',
        })
      );
      return;
    }

    const finalName =
      name.trim() ||
      testResult?.serverName ||
      `${serverType === 'jellyfin' ? 'Jellyfin' : 'Emby'} Server`;

    const profileToSave: MediaServerProfile = {
      id: editingId || uuidv4(),
      name: finalName,
      serverType,
      serverUrl: cleanUrl,
      apiKey: cleanKey,
      username: username.trim() || undefined,
      adminUserId,
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

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('media_server.manage_title', { defaultValue: 'Manage Media Servers' })}
      size="lg"
      footer={
        <div className="media-server-modal-footer">
          {isEditing ? (
            <>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setIsEditing(false)}
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
                    <RefreshCw size={14} className="spin" />
                    <span>{t('media_server.testing', { defaultValue: 'Testing...' })}</span>
                  </>
                ) : (
                  <span>
                    {t('media_server.test_connection', { defaultValue: 'Test Connection' })}
                  </span>
                )}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={!serverUrl.trim() || !apiKey.trim()}
                onClick={handleSaveProfile}
              >
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
      <div className="media-server-manage-modal-body">
        {!isEditing ? (
          <div className="media-server-list-view">
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
              <div className="media-server-empty-state">
                <Tv size={36} className="media-server-empty-icon" />
                <p>
                  {t('media_server.no_servers_configured', {
                    defaultValue:
                      'No media servers configured. Add an Emby or Jellyfin server to get started.',
                  })}
                </p>
                <button type="button" className="btn btn-primary" onClick={startAddServer}>
                  <Plus size={14} style={{ marginRight: 6 }} />
                  <span>{t('media_server.add_server', { defaultValue: 'Add Server' })}</span>
                </button>
              </div>
            ) : (
              <div className="media-server-cards-grid">
                {profiles.map((profile) => {
                  const isActive = profile.id === activeProfileId;
                  const status = perServerStatus[profile.id];

                  return (
                    <div
                      key={profile.id}
                      className={`media-server-card ${isActive ? 'active-server' : ''}`}
                    >
                      <div className="media-server-card-main">
                        <div className="media-server-card-header">
                          <span className={`media-server-type-badge ${profile.serverType}`}>
                            {profile.serverType === 'jellyfin' ? 'Jellyfin' : 'Emby'}
                          </span>
                          <h4 className="media-server-card-name" title={profile.name}>
                            {profile.name}
                          </h4>
                          {isActive && (
                            <span className="media-server-active-pill">
                              {t('media_server.active', { defaultValue: 'Active' })}
                            </span>
                          )}
                        </div>

                        <div className="media-server-card-meta">
                          <span className="media-server-card-url" title={profile.serverUrl}>
                            {profile.serverUrl}
                          </span>
                          {profile.username && (
                            <span className="media-server-card-user">({profile.username})</span>
                          )}
                        </div>

                        {status && (
                          <div className={`media-server-test-status ${status.status}`}>
                            {status.status === 'testing' && (
                              <>
                                <RefreshCw size={12} className="spin" />
                                <span>
                                  {t('media_server.testing', { defaultValue: 'Testing...' })}
                                </span>
                              </>
                            )}
                            {status.status === 'ok' && (
                              <>
                                <CheckCircle2 size={12} />
                                <span>
                                  {t('media_server.connected', { defaultValue: 'Connected' })}{' '}
                                  {status.message}
                                </span>
                              </>
                            )}
                            {status.status === 'error' && (
                              <>
                                <AlertCircle size={12} />
                                <span title={status.message}>
                                  {t('common.error', { defaultValue: 'Error' })}: {status.message}
                                </span>
                              </>
                            )}
                          </div>
                        )}
                      </div>

                      <div className="media-server-card-actions">
                        {!isActive && (
                          <button
                            type="button"
                            className="btn btn-xs btn-secondary"
                            onClick={() => onSelectActiveProfile(profile.id)}
                          >
                            {t('media_server.set_active', { defaultValue: 'Select' })}
                          </button>
                        )}
                        <button
                          type="button"
                          className="btn btn-xs btn-secondary"
                          onClick={() => handleTestInList(profile)}
                        >
                          {t('media_server.test', { defaultValue: 'Test' })}
                        </button>
                        <button
                          type="button"
                          className="btn btn-icon btn-xs"
                          onClick={() => startEditServer(profile)}
                          title={t('common.edit', { defaultValue: 'Edit' })}
                        >
                          <Edit2 size={13} />
                        </button>
                        <button
                          type="button"
                          className="btn btn-icon btn-xs btn-danger"
                          onClick={() => handleDelete(profile.id)}
                          title={t('common.delete', { defaultValue: 'Delete' })}
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        ) : (
          <div className="media-server-form-view">
            <h4 className="media-server-form-title">
              {editingId
                ? t('media_server.edit_server', { defaultValue: 'Edit Media Server' })
                : t('media_server.add_server', { defaultValue: 'Add Media Server' })}
            </h4>

            {formError && (
              <div className="media-server-form-alert error" role="alert">
                <AlertCircle size={15} style={{ flexShrink: 0 }} />
                <span>{formError}</span>
              </div>
            )}

            {testResult && (
              <div className="media-server-form-alert success">
                <CheckCircle2 size={15} style={{ flexShrink: 0 }} />
                <span>
                  {t('media_server.connection_successful', {
                    defaultValue: 'Connected successfully to',
                  })}{' '}
                  <strong>{testResult.serverName}</strong> (v{testResult.version})
                </span>
              </div>
            )}

            <div className="form-group">
              <label className="form-label">
                {t('media_server.server_type', { defaultValue: 'Server Type' })}
              </label>
              <div className="media-server-type-selector">
                <button
                  type="button"
                  className={`type-btn emby ${serverType === 'emby' ? 'selected' : ''}`}
                  onClick={() => setServerType('emby')}
                >
                  <Tv size={15} />
                  <span>Emby</span>
                </button>
                <button
                  type="button"
                  className={`type-btn jellyfin ${serverType === 'jellyfin' ? 'selected' : ''}`}
                  onClick={() => setServerType('jellyfin')}
                >
                  <Server size={15} />
                  <span>Jellyfin</span>
                </button>
              </div>
            </div>

            <div className="form-group">
              <label className="form-label" htmlFor="server-url-input">
                {t('media_server.server_url', { defaultValue: 'Server URL' })}
              </label>
              <input
                id="server-url-input"
                type="text"
                className="form-input"
                placeholder={
                  serverType === 'jellyfin'
                    ? 'http://192.168.1.100:8096'
                    : 'http://192.168.1.100:8096'
                }
                value={serverUrl}
                onChange={(e) => setServerUrl(e.target.value)}
              />
            </div>

            <div className="form-group checkbox-group">
              <label className="checkbox-label">
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
            </div>

            <div className="form-group">
              <label className="form-label">
                {t('media_server.auth_method', { defaultValue: 'Authentication Method' })}
              </label>
              <div className="media-server-tab-selector">
                <button
                  type="button"
                  className={`tab-btn ${authMethod === 'login' ? 'active' : ''}`}
                  onClick={() => setAuthMethod('login')}
                >
                  {t('media_server.account_login', { defaultValue: 'Account Login' })}
                </button>
                <button
                  type="button"
                  className={`tab-btn ${authMethod === 'apikey' ? 'active' : ''}`}
                  onClick={() => setAuthMethod('apikey')}
                >
                  {t('media_server.manual_api_key', { defaultValue: 'API Key / Token' })}
                </button>
              </div>
            </div>

            {authMethod === 'login' ? (
              <div className="media-server-login-section">
                <div className="form-group">
                  <label className="form-label" htmlFor="username-input">
                    {t('media_server.username', { defaultValue: 'Username' })}
                  </label>
                  <input
                    id="username-input"
                    type="text"
                    className="form-input"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    placeholder="admin"
                  />
                </div>
                <div className="form-group">
                  <label className="form-label" htmlFor="password-input">
                    {t('media_server.password', { defaultValue: 'Password' })}
                  </label>
                  <input
                    id="password-input"
                    type="password"
                    className="form-input"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="••••••••"
                  />
                </div>
                <button
                  type="button"
                  className="btn btn-sm btn-secondary"
                  disabled={isLoggingIn || !serverUrl.trim() || !username.trim()}
                  onClick={handleLoginAction}
                >
                  {isLoggingIn ? (
                    <>
                      <RefreshCw size={13} className="spin" />
                      <span>{t('media_server.logging_in', { defaultValue: 'Logging in...' })}</span>
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
            ) : (
              <div className="form-group">
                <label className="form-label" htmlFor="api-key-input">
                  {t('media_server.api_key_or_token', { defaultValue: 'API Key / Access Token' })}
                </label>
                <input
                  id="api-key-input"
                  type="password"
                  className="form-input"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Paste your API key or access token here"
                />
              </div>
            )}

            <div className="form-group">
              <label className="form-label" htmlFor="profile-name-input">
                {t('media_server.profile_name', { defaultValue: 'Display Name' })}
              </label>
              <input
                id="profile-name-input"
                type="text"
                className="form-input"
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
