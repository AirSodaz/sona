import { AlertCircle, RefreshCw } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import type { EmbyConnectionConfig } from '../../../../types/emby';
import { Modal } from '../../../Modal';

export interface EmbyServerConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  config: EmbyConnectionConfig;
  onChangeConfig: React.Dispatch<React.SetStateAction<EmbyConnectionConfig>>;
  isConnecting: boolean;
  connectionError: string | null;
  onConnect: () => void | Promise<void>;
}

export function EmbyServerConfigModal({
  isOpen,
  onClose,
  config,
  onChangeConfig,
  isConnecting,
  connectionError,
  onConnect,
}: EmbyServerConfigModalProps): React.JSX.Element {
  const { t } = useTranslation();

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('emby.modal_config_title', {
        defaultValue: 'Configure Emby Server Connection',
      })}
      size="md"
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
            disabled={isConnecting || !config.serverUrl.trim() || !config.apiKey.trim()}
            onClick={() => {
              void onConnect();
            }}
          >
            {isConnecting ? (
              <>
                <RefreshCw size={14} className="spin" style={{ marginRight: 6 }} />
                {t('common.connecting', { defaultValue: 'Connecting...' })}
              </>
            ) : (
              t('emby.connect_and_save', { defaultValue: 'Test & Save' })
            )}
          </button>
        </div>
      }
    >
      <div className="emby-config-modal-body">
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
            type="text"
            className="emby-input"
            placeholder="http://192.168.1.100:8096"
            value={config.serverUrl}
            onChange={(e) => onChangeConfig((prev) => ({ ...prev, serverUrl: e.target.value }))}
            disabled={isConnecting}
          />
        </label>

        <label className="emby-form-field">
          <span className="emby-form-label">
            {t('emby.modal_key_label', { defaultValue: 'API Key (Admin token)' })}
          </span>
          <input
            type="password"
            className="emby-input"
            placeholder="Emby API Key"
            value={config.apiKey}
            onChange={(e) => onChangeConfig((prev) => ({ ...prev, apiKey: e.target.value }))}
            disabled={isConnecting}
          />
        </label>

        <label className="emby-checkbox-field">
          <input
            type="checkbox"
            checked={config.allowInvalidCerts}
            onChange={(e) =>
              onChangeConfig((prev) => ({ ...prev, allowInvalidCerts: e.target.checked }))
            }
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
