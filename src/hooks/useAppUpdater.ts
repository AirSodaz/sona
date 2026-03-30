import { useState, useCallback } from 'react';
import { check, Update } from '@tauri-apps/plugin-updater';
import { useDialogStore } from '../stores/dialogStore';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';
import { buildErrorDialogOptions } from '../utils/errorUtils';

export type UpdateStatus =
    | 'idle'
    | 'checking'
    | 'available'
    | 'uptodate'
    | 'downloading'
    | 'installing'
    | 'downloaded' // Ready to restart
    | 'error';

interface UseAppUpdaterReturn {
    status: UpdateStatus;
    error: string | null;
    updateInfo: Update | null;
    checkUpdate: (manual?: boolean) => Promise<void>;
    installUpdate: () => Promise<void>;
    progress: number;
}

export function useAppUpdater(): UseAppUpdaterReturn {
    const [status, setStatus] = useState<UpdateStatus>('idle');
    const [error, setError] = useState<string | null>(null);
    const [updateInfo, setUpdateInfo] = useState<Update | null>(null);
    const [progress, setProgress] = useState<number>(0);
    const confirm = useDialogStore((state) => state.confirm);
    const { t } = useTranslation();

    const showErrorPopup = async (err: unknown) => {
        const { title, message, details } = buildErrorDialogOptions(t, {
            code: 'update.failed',
            messageKey: 'errors.update.failed',
            cause: err,
        });
        const shouldDownload = await confirm(message, {
            title,
            details,
            variant: 'error',
            confirmLabel: t('settings.update_download_manually', { defaultValue: 'Download Manually' }),
            cancelLabel: t('common.cancel', { defaultValue: 'Cancel' })
        });

        if (shouldDownload) {
            try {
                await openUrl('https://github.com/AirSodaz/sona/releases/latest');
            } catch (openErr) {
                console.error('Failed to open URL:', openErr);
            }
        }
    };

    const checkUpdate = useCallback(async (manual: boolean = false) => {
        setStatus('checking');
        setError(null);
        setProgress(0);
        try {
            const update = await check();
            if (update) {
                setUpdateInfo(update);
                setStatus('available');
            } else {
                setUpdateInfo(null);
                setStatus('uptodate');
            }
        } catch (err) {
            console.error('Update check failed:', err);
            if (manual) {
                setStatus('idle');
                await showErrorPopup(err);
            } else {
                if (err instanceof Error) {
                    setError(err.message);
                } else {
                    setError(String(err));
                }
                setStatus('error');
            }
        }
    }, [confirm, t]);

    const installUpdate = useCallback(async () => {
        if (!updateInfo) return;

        setStatus('downloading');
        setError(null);
        setProgress(0);

        try {
            let downloaded = 0;
            let contentLength = 0;

            await updateInfo.downloadAndInstall((event) => {
                if (event.event === 'Started') {
                    contentLength = event.data.contentLength || 0;
                } else if (event.event === 'Progress') {
                    downloaded += event.data.chunkLength;
                    if (contentLength > 0) {
                        setProgress(Math.round((downloaded / contentLength) * 100));
                    }
                } else if (event.event === 'Finished') {
                    setStatus('installing');
                }
            });

            // If the app hasn't restarted yet, it means the update is downloaded and installed (staged).
            // Usually downloadAndInstall restarts the app.
            setStatus('downloaded');

        } catch (err) {
            console.error('Update installation failed:', err);
            setStatus('idle');
            await showErrorPopup(err);
        }
    }, [updateInfo, confirm, t]);

    return {
        status,
        error,
        updateInfo,
        checkUpdate,
        installUpdate,
        progress
    };
}
