import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { forceExitWithGuard } from '../services/quitGuard';
import { SettingsTab } from './useSettingsLogic';
import { logger } from '../utils/logger';
import { useAppUpdaterStore } from '../stores/appUpdaterStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { updateTrayMenu } from '../services/tauri/app';
import { TauriEvent } from '../services/tauri/events';

/**
 * Hook to handle system tray events.
 *
 * Routes tray actions into the existing settings, caption, updater, and quit flows.
 */
export function useTrayHandling(
    setIsSettingsOpen: (open: boolean) => void,
    setSettingsInitialTab: (tab: SettingsTab) => void
) {
    const { t, i18n } = useTranslation();
    const isCaptionMode = useTranscriptRuntimeStore((state) => state.isCaptionMode);

    // Update tray menu language when locale changes or caption state changes
    useEffect(() => {
        const updateTray = async () => {
            try {
                await updateTrayMenu({
                    showText: t('tray.show'),
                    settingsText: t('tray.settings'),
                    updatesText: t('tray.check_updates'),
                    quitText: t('tray.quit'),
                    captionText: t('tray.live_caption'),
                    captionChecked: isCaptionMode
                });
            } catch (err) {
                logger.warn('Failed to update tray menu language:', err);
            }
        };
        updateTray();
    }, [i18n.language, t, isCaptionMode]);

    useEffect(() => {
        let isMounted = true;
        const unlistenFunctions: UnlistenFn[] = [];

        const setupListeners = async () => {
            try {
                const unlistenOpenSettings = await listen(TauriEvent.tray.openSettings, () => {
                    if (!isMounted) return;
                    setSettingsInitialTab('general');
                    setIsSettingsOpen(true);
                });
                if (isMounted) unlistenFunctions.push(unlistenOpenSettings);
                else unlistenOpenSettings();

                const unlistenToggleCaption = await listen(TauriEvent.tray.toggleCaption, () => {
                    if (!isMounted) return;
                    const runtimeStore = useTranscriptRuntimeStore.getState();
                    runtimeStore.setIsCaptionMode(!runtimeStore.isCaptionMode);
                });
                if (isMounted) unlistenFunctions.push(unlistenToggleCaption);
                else unlistenToggleCaption();

                const unlistenCheckUpdates = await listen(TauriEvent.tray.checkUpdates, () => {
                    if (!isMounted) return;
                    setSettingsInitialTab('about');
                    setIsSettingsOpen(true);
                    void useAppUpdaterStore.getState().checkUpdate(true);
                });
                if (isMounted) unlistenFunctions.push(unlistenCheckUpdates);
                else unlistenCheckUpdates();

                const unlistenRequestQuit = await listen(TauriEvent.tray.requestQuit, async () => {
                    if (!isMounted) return;
                    try {
                        await forceExitWithGuard();
                    } catch (error) {
                        logger.error('Failed to handle quit request:', error);
                    }
                });
                if (isMounted) unlistenFunctions.push(unlistenRequestQuit);
                else unlistenRequestQuit();

            } catch (error) {
                logger.error('Failed to setup tray listeners:', error);
            }
        };

        setupListeners();

        return () => {
            isMounted = false;
            unlistenFunctions.forEach(fn => fn());
        };
    }, [setIsSettingsOpen, setSettingsInitialTab]);
}
