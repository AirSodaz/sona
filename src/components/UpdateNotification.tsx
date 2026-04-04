import React, { useEffect, useState } from 'react';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useTranslation } from 'react-i18next';
import { Download, X } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import { useErrorDialogStore } from '../stores/errorDialogStore';
import { openUrl } from '@tauri-apps/plugin-opener';
import { buildErrorDialogViewModel } from '../utils/errorUtils';

export function UpdateNotification(): React.JSX.Element | null {
    const { t } = useTranslation();
    const config = useConfigStore((state) => state.config);
    const showError = useErrorDialogStore((state) => state.showError);
    const [updateAvailable, setUpdateAvailable] = useState<any>(null);
    const [isInstalling, setIsInstalling] = useState(false);
    const [isVisible, setIsVisible] = useState(false);

    useEffect(() => {
        const checkForUpdates = async () => {
            // Only check if auto-check is enabled
            if (!config.autoCheckUpdates) return;

            try {
                const update = await check();
                if (update?.available) {
                    setUpdateAvailable(update);
                    setIsVisible(true);
                }
            } catch (err) {
                console.error('Failed to check for updates:', err);
                // Do not show error to user on auto-check to avoid annoyance
            }
        };

        // Delay check slightly to not impact startup performance
        const timer = setTimeout(() => {
            checkForUpdates();
        }, 5000); // 5 seconds delay

        return () => clearTimeout(timer);
    }, [config.autoCheckUpdates]);

    const handleUpdate = async () => {
        if (!updateAvailable) return;

        try {
            setIsInstalling(true);
            await updateAvailable.downloadAndInstall();
            await relaunch();
        } catch (err: any) {
            console.error('Update failed:', err);
            setIsInstalling(false);
            const result = await showError(buildErrorDialogViewModel(t, {
                code: 'update.failed',
                messageKey: 'errors.update.failed',
                cause: err,
                primaryActionLabelKey: 'settings.update_download_manually',
            }));

            if (result === 'primary') {
                try {
                    await openUrl('https://github.com/AirSodaz/sona/releases/latest');
                } catch (openErr) {
                    console.error('Failed to open URL:', openErr);
                }
            }
        }
    };

    const handleDismiss = () => {
        setIsVisible(false);
    };

    if (!isVisible || !updateAvailable) return null;

    return (
        <div className="update-notification" role="alert">
            <div className="update-notification-header">
                <div className="update-notification-title">
                    <Download size={18} style={{ color: 'var(--color-info)' }} />
                    <span>{t('settings.update_available', { version: updateAvailable.version })}</span>
                </div>
                <button
                    className="update-notification-close"
                    onClick={handleDismiss}
                    aria-label={t('common.close')}
                >
                    <X size={16} />
                </button>
            </div>

            <div className="update-notification-body">
                {updateAvailable.body ? (
                    <div style={{ maxHeight: '100px', overflowY: 'auto', whiteSpace: 'pre-wrap', fontSize: '0.8rem' }}>
                        {updateAvailable.body}
                    </div>
                ) : (
                   <div>{t('settings.update_desc_default')}</div>
                )}
            </div>

            <div className="update-notification-actions">
                <button
                    className="btn btn-secondary"
                    onClick={handleDismiss}
                    disabled={isInstalling}
                >
                    {t('common.cancel')}
                </button>
                <button
                    className="btn btn-primary"
                    onClick={handleUpdate}
                    disabled={isInstalling}
                >
                    {isInstalling ? t('settings.update_installing') : t('settings.update_btn_install')}
                </button>
            </div>
        </div>
    );
}
