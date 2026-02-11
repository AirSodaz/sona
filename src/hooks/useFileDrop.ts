import React, { useState, useCallback, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Event } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useDialogStore } from '../stores/dialogStore';

/**
 * Hook to handle file drag and drop interactions with Tauri.
 *
 * Manages both Tauri-specific system drag-and-drop events and standard React
 * drag events for visual feedback.
 *
 * @param onFilesDropped Callback function invoked when valid files are dropped.
 * @param acceptedExtensions List of accepted file extensions (e.g., ['.wav', '.mp3']).
 * @return Object containing drag state and event handlers.
 */
export function useFileDrop(
    onFilesDropped: (files: string[]) => void,
    acceptedExtensions: string[]
): {
    isDragOver: boolean;
    handleDrop: (e: React.DragEvent) => void;
    handleDragOver: (e: React.DragEvent) => void;
    handleDragEnter: (e: React.DragEvent) => void;
    handleDragLeave: (e: React.DragEvent) => void;
} {
    const [isDragOver, setIsDragOver] = useState(false);
    const { alert } = useDialogStore();
    const { t } = useTranslation();

    const handleTauriDrop = useCallback((payload: unknown) => {
        let files: string[] = [];

        if (Array.isArray(payload)) {
            files = payload as string[];
        } else if (payload && typeof payload === 'object' && 'paths' in payload && Array.isArray((payload as { paths: unknown }).paths)) {
            files = (payload as { paths: string[] }).paths;
        }

        if (files && files.length > 0) {
            // Validate all files
            const validFiles: string[] = [];
            const invalidFiles: string[] = [];

            files.forEach((filePath) => {
                const ext = filePath.split('.').pop()?.toLowerCase();
                const isSupported = acceptedExtensions.some(e => e.replace('.', '') === ext);
                if (isSupported) {
                    validFiles.push(filePath);
                } else {
                    invalidFiles.push(filePath);
                }
            });

            if (invalidFiles.length > 0) {
                alert(t('batch.unsupported_format', { formats: acceptedExtensions.join(', ') }), { variant: 'error' });
            }

            if (validFiles.length > 0) {
                onFilesDropped(validFiles);
            }
        } else {
            console.warn('File drop event received but payload is empty or invalid.');
        }
        setIsDragOver(false);
    }, [acceptedExtensions, alert, onFilesDropped, t]);

    // Tauri File Drop Event Listener
    useEffect(() => {
        let mounted = true;
        const unlisteners: Array<() => void> = [];

        const setupListeners = async () => {
            const appWindow = getCurrentWindow();

            // Only listen to tauri://drag-drop (Tauri v2)
            const unlistenDrop = await appWindow.listen('tauri://drag-drop', (event: Event<unknown>) => {
                if (mounted) {
                    handleTauriDrop(event.payload);
                }
            });
            if (mounted) unlisteners.push(unlistenDrop);

            const unlistenHover = await appWindow.listen('tauri://drag-enter', () => {
                if (mounted) setIsDragOver(true);
            });
            if (mounted) unlisteners.push(unlistenHover);

            const unlistenCancelled = await appWindow.listen('tauri://drag-leave', () => {
                if (mounted) setIsDragOver(false);
            });
            if (mounted) unlisteners.push(unlistenCancelled);
        };

        setupListeners();

        return () => {
            mounted = false;
            unlisteners.forEach((unlisten) => unlisten());
        };
    }, [handleTauriDrop]);

    // React DnD handlers (mainly for visual feedback, as Tauri handles the actual drop)
    const handleDrop = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        setIsDragOver(false);
    }, []);

    const handleDragOver = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        if (!isDragOver) setIsDragOver(true);
    }, [isDragOver]);

    const handleDragEnter = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        setIsDragOver(true);
    }, []);

    const handleDragLeave = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        setIsDragOver(false);
    }, []);

    return {
        isDragOver,
        handleDrop,
        handleDragOver,
        handleDragEnter,
        handleDragLeave
    };
}
