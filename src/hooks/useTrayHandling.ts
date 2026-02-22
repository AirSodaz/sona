import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';

/**
 * Hook to handle system tray events.
 *
 * Listens for:
 * - 'open-settings': Opens settings modal on default tab.
 * - 'check-updates': Opens settings modal on update tab and triggers check.
 * - 'request-quit': Checks for active tasks and confirms before quitting.
 */
export function useTrayHandling(
    setIsSettingsOpen: (open: boolean) => void,
    setSettingsInitialTab: (tab: 'general' | 'about') => void
) {
    const { confirm } = useDialogStore();
    const { t, i18n } = useTranslation();
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);

    // Update tray menu language when locale changes or caption state changes
    useEffect(() => {
        const updateTray = async () => {
            try {
                await invoke('update_tray_menu', {
                    showText: t('tray.show'),
                    settingsText: t('tray.settings'),
                    updatesText: t('tray.check_updates'),
                    quitText: t('tray.quit'),
                    captionText: isCaptionMode ? t('tray.stop_caption') : t('tray.start_caption')
                });
            } catch (err) {
                console.warn('Failed to update tray menu language:', err);
            }
        };
        updateTray();
    }, [i18n.language, t, isCaptionMode]);

    useEffect(() => {
        let isMounted = true;
        let unlistenFunctions: UnlistenFn[] = [];

        const setupListeners = async () => {
            try {
                const unlistenOpenSettings = await listen('open-settings', () => {
                    if (!isMounted) return;
                    setSettingsInitialTab('general');
                    setIsSettingsOpen(true);
                });
                if (isMounted) unlistenFunctions.push(unlistenOpenSettings);
                else unlistenOpenSettings();

                const unlistenToggleCaption = await listen('toggle-caption', () => {
                    if (!isMounted) return;
                    const currentMode = useTranscriptStore.getState().isCaptionMode;
                    useTranscriptStore.getState().setIsCaptionMode(!currentMode);
                });
                if (isMounted) unlistenFunctions.push(unlistenToggleCaption);
                else unlistenToggleCaption();

                const unlistenCheckUpdates = await listen('check-updates', () => {
                    if (!isMounted) return;
                    setSettingsInitialTab('about');
                    setIsSettingsOpen(true);
                    // Emit a custom window event that SettingsAboutTab can listen to
                    setTimeout(() => {
                        window.dispatchEvent(new CustomEvent('trigger-update-check'));
                    }, 500);
                });
                if (isMounted) unlistenFunctions.push(unlistenCheckUpdates);
                else unlistenCheckUpdates();

                const unlistenRequestQuit = await listen('request-quit', async () => {
                    if (!isMounted) return;
                    // Access store state directly (not via hook) to get fresh values
                    const state = useTranscriptStore.getState();
                    const isRecording = state.isRecording;
                    const isProcessing = state.processingStatus === 'processing';

                    let hasDownloads = false;
                    try {
                        hasDownloads = await invoke<boolean>('has_active_downloads');
                    } catch (e) {
                        console.error('Failed to check downloads:', e);
                    }

                    if (isRecording || isProcessing || hasDownloads) {
                        const confirmed = await confirm(
                            t('tray.quit_warning_message'),
                            {
                                title: t('tray.quit_warning_title'),
                                variant: 'warning',
                                confirmLabel: t('tray.quit_confirm'),
                                cancelLabel: t('common.cancel')
                            }
                        );

                        if (confirmed) {
                            await invoke('force_exit');
                        }
                    } else {
                        await invoke('force_exit');
                    }
                });
                if (isMounted) unlistenFunctions.push(unlistenRequestQuit);
                else unlistenRequestQuit();

            } catch (error) {
                console.error('Failed to setup tray listeners:', error);
            }
        };

        setupListeners();

        return () => {
            isMounted = false;
            unlistenFunctions.forEach(fn => fn());
        };
    }, [setIsSettingsOpen, setSettingsInitialTab, confirm]);
}
