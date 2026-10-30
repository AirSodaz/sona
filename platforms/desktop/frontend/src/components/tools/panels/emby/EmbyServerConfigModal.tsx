import { AlertCircle, RefreshCw } from 'lucide-react';
import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { EmbyConnectionConfig, EmbyLoginPayload } from '../../../../types/emby';
import { Modal } from '../../../Modal';

export interface EmbyServerConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  config: EmbyConnectionConfig;
  onChangeConfig: React.Dispatch<React.SetStateAction<EmbyConnectionConfig>>;
  isConnecting: boolean;
  connectionError: string | null;
  onConnect: (config?: EmbyConnectionConfig) => void | Promise<void>;
  onLogin?: (payload: EmbyLoginPayload) => void | Promise<void>;
}

export function EmbyServerConfigModal({
  isOpen,
  onClose,
  config,
  onChangeConfig: _onChangeConfig,
  isConnecting,
  connectionError,
  onConnect,
  onLogin,
}: EmbyServerConfigModalProps): React.JSX.Element {
  const { t } = useTranslation();
  const urlInputRef = useRef<HTMLInputElement>(null);

  const [serverUrl, setServerUrl] = useState(config.serverUrl || '');
  const [username, setUsername] = useState(config.username || '');
  const [password, setPassword] = useState('');
  const [allowInvalidCerts, setAllowInvalidCerts] = useState(config.allowInvalidCerts || false);

  useEffect(() => {
    if (isOpen) {
      setServerUrl(config.serverUrl || '');
      setUsername(config.username || '');
      setPassword('');
      setAllowInvalidCerts(config.allowInvalidCerts || false);
    }
  }, [isOpen, config.serverUrl, config.username, config.allowInvalidCerts]);

  const cleanUrl = serverUrl.trim().replace(/\/+$/, '');
  const cleanUser = username.trim();
  const isUrlChanged = cleanUrl !== (config.serverUrl || '').trim().replace(/\/+$/, '');
  const isSubmitDisabled = isConnecting || !cleanUrl || !cleanUser;

  const handleSubmit = () => {
    if (isSubmitDisabled) return;

    if (
      onLogin &&
      (password || !config.apiKey || isUrlChanged || cleanUser !== (config.username || ''))
    ) {
      if (!cleanUser) return;
      void onLogin({
        serverUrl: cleanUrl,
        username: cleanUser,
        password: password || undefined,
        allowInvalidCerts,
      });
      return;
    }

    if (config.apiKey) {
      const updated: EmbyConnectionConfig = {
        ...config,
        serverUrl: cleanUrl,
        username: cleanUser || config.username,
        allowInvalidCerts,
      };
      void onConnect(updated);
      return;
    }

    if (onLogin && cleanUser) {
      void onLogin({
        serverUrl: cleanUrl,
        username: cleanUser,
        password: password || undefined,
        allowInvalidCerts,
      });
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.nativeEvent.isComposing) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('emby.modal_config_title', {
        defaultValue: 'Configure Emby Server Connection',
      })}
      size="md"
      initialFocusRef={urlInputRef}
      closeOnOverlayClick={!isConnecting}
      closeOnEsc={!isConnecting}
      hideCloseButton={isConnecting}
      footer={
        <div className="emby-modal-footer-actions">
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onClose}
            disabled={isConnecting}
          >
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={isSubmitDisabled}
            onClick={handleSubmit}
          >
            {isConnecting ? (
              <>
                <RefreshCw size={14} className="spin" style={{ marginRight: 6 }} />
                {t('common.connecting', { defaultValue: 'Connecting...' })}
              </>
            ) : !config.apiKey ||
              password ||
              cleanUser !== (config.username || '') ||
              isUrlChanged ? (
              t('emby.login_and_connect', { defaultValue: 'Login & Connect' })
            ) : (
              t('emby.test_connection', { defaultValue: 'Test & Connect' })
            )}
          </button>
        </div>
      }
    >
      <div className="emby-config-modal-body" onKeyDown={handleKeyDown}>
        {connectionError && (
          <div className="emby-alert-box emby-alert-error" role="alert">
            <AlertCircle size={16} />
            <span>{connectionError}</span>
          </div>
        )}

        <label className="emby-form-field">
          <span className="emby-form-label">
            {t('emby.modal_url_label', {
              defaultValue: 'Server URL (including port and path)',
            })}
          </span>
          <input
            ref={urlInputRef}
            type="text"
            className="emby-input"
            placeholder="http://192.168.1.100:8096"
            value={serverUrl}
            onChange={(e) => setServerUrl(e.target.value)}
            onBlur={() => setServerUrl((prev) => prev.trim())}
            disabled={isConnecting}
          />
        </label>

        <label className="emby-form-field">
          <span className="emby-form-label">
            {t('emby.modal_username_label', { defaultValue: 'Admin Username' })}
          </span>
          <input
            type="text"
            className="emby-input"
            placeholder={t('emby.modal_username_placeholder', { defaultValue: 'e.g. admin' })}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            onBlur={() => setUsername((prev) => prev.trim())}
            disabled={isConnecting}
            autoComplete="username"
          />
        </label>

        <label className="emby-form-field">
          <span className="emby-form-label">
            {t('emby.modal_password_label', { defaultValue: 'Admin Password' })}
          </span>
          <input
            type="password"
            className="emby-input"
            placeholder={
              config.apiKey
                ? t('emby.modal_password_placeholder_saved', {
                    defaultValue: 'Leave blank to keep existing login',
                  })
                : t('emby.modal_password_placeholder', {
                    defaultValue: 'Password (leave blank if none)',
                  })
            }
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            disabled={isConnecting}
            autoComplete="current-password"
          />
        </label>

        <label className="emby-checkbox-field">
          <input
            type="checkbox"
            checked={allowInvalidCerts}
            onChange={(e) => setAllowInvalidCerts(e.target.checked)}
            disabled={isConnecting}
          />
          <span>
            {t('emby.modal_cert_label', {
              defaultValue: 'Allow self-signed or invalid SSL certificates (LAN HTTPS)',
            })}
          </span>
        </label>
      </div>
    </Modal>
  );
}
