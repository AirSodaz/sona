import React from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Download, X } from 'lucide-react';
import { useAppUpdater } from '../hooks/useAppUpdater';

export function UpdateNotification(): React.JSX.Element | null {
  const { t } = useTranslation();
  const {
    status,
    updateInfo,
    progress,
    notificationVisible,
    installUpdate,
    dismissNotification,
    relaunchToUpdate,
  } = useAppUpdater();

  const isBusy = status === 'downloading' || status === 'installing';

  if (!notificationVisible || !updateInfo) {
    return null;
  }

  const renderBody = () => {
    if (status === 'downloading' || status === 'installing') {
      return (
        <div className="update-progress-container">
          <div className="update-progress-header">
            <span>{status === 'downloading' ? t('settings.update_downloading') : t('settings.update_installing')}</span>
            <span>{progress}%</span>
          </div>
          <div className="progress-bar">
            <div className="progress-bar-fill" style={{ width: `${progress}%` }} />
          </div>
        </div>
      );
    }

    if (status === 'downloaded') {
      return (
        <div className="update-status success">
          <Check size={18} />
          <span>{t('settings.update_relaunch')}</span>
        </div>
      );
    }

    if (updateInfo.body) {
      return (
        <div style={{ maxHeight: '100px', overflowY: 'auto', whiteSpace: 'pre-wrap', fontSize: '0.8rem' }}>
          {updateInfo.body}
        </div>
      );
    }

    return <div>{t('settings.update_desc_default')}</div>;
  };

  let actionLabel;
  if (status === 'downloaded') {
    actionLabel = t('settings.update_btn_relaunch');
  } else if (isBusy) {
    actionLabel = status === 'downloading' ? t('settings.update_downloading') : t('settings.update_installing');
  } else {
    actionLabel = t('settings.update_btn_install');
  }

  return (
    <div className="update-notification" role="alert">
      <div className="update-notification-header">
        <div className="update-notification-title">
          <Download size={18} style={{ color: 'var(--color-info)' }} />
          <span>{t('settings.update_available', { version: updateInfo.version })}</span>
        </div>
        <button
          className="update-notification-close"
          onClick={dismissNotification}
          aria-label={t('common.close')}
          disabled={isBusy}
        >
          <X size={16} />
        </button>
      </div>

      <div className="update-notification-body">
        {renderBody()}
      </div>

      <div className="update-notification-actions">
        <button
          className="btn btn-secondary"
          onClick={dismissNotification}
          disabled={isBusy}
        >
          {t('common.cancel')}
        </button>
        <button
          className="btn btn-primary"
          onClick={status === 'downloaded' ? relaunchToUpdate : installUpdate}
          disabled={isBusy}
        >
          {actionLabel}
        </button>
      </div>
    </div>
  );
}
