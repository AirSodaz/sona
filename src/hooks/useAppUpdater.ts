import { useState, useCallback } from 'react';
import { check, Update } from '@tauri-apps/plugin-updater';

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
    checkUpdate: () => Promise<void>;
    installUpdate: () => Promise<void>;
    progress: number;
}

export function useAppUpdater(): UseAppUpdaterReturn {
    const [status, setStatus] = useState<UpdateStatus>('idle');
    const [error, setError] = useState<string | null>(null);
    const [updateInfo, setUpdateInfo] = useState<Update | null>(null);
    const [progress, setProgress] = useState<number>(0);

    const checkUpdate = useCallback(async () => {
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
            setError(err instanceof Error ? err.message : String(err));
            setStatus('error');
        }
    }, []);

    const installUpdate = useCallback(async () => {
        if (!updateInfo) return;

        setStatus('downloading');
        setError(null);
        setProgress(0);

        try {
            let downloaded = 0;
            let contentLength = 0;

            await updateInfo.downloadAndInstall((event) => {
                switch (event.event) {
                    case 'Started':
                        contentLength = event.data.contentLength || 0;
                        break;
                    case 'Progress':
                        downloaded += event.data.chunkLength;
                        if (contentLength > 0) {
                            setProgress(Math.round((downloaded / contentLength) * 100));
                        }
                        break;
                    case 'Finished':
                        setStatus('installing');
                        break;
                }
            });

            // If the app hasn't restarted yet, it means the update is downloaded and installed (staged).
            // Usually downloadAndInstall restarts the app.
            setStatus('downloaded');

        } catch (err) {
            console.error('Update installation failed:', err);
            setError(err instanceof Error ? err.message : String(err));
            setStatus('error');
        }
    }, [updateInfo]);

    return {
        status,
        error,
        updateInfo,
        checkUpdate,
        installUpdate,
        progress
    };
}
