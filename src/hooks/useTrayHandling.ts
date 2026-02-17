import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
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
                            'You have active tasks running (recording, downloading, or processing). Quitting now will terminate them.\n\nAre you sure you want to quit?',
                            {
                                title: 'Active Tasks Running',
                                variant: 'warning',
                                confirmLabel: 'Quit Anyway',
                                cancelLabel: 'Cancel'
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
