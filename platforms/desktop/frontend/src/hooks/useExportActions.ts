import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useDialogStore } from '../stores/dialogStore';
import type { ExportFormat, ExportMode } from '../utils/exportFormats';
import { exportTranscriptToDirectory } from '../services/exportService';
import { logger } from '../utils/logger';
import { openDialog } from '../services/tauri/platform/dialog';

interface UseExportActionsProps {
    isOpen: boolean;
    onSuccess: () => void;
}

export function useExportActions({ isOpen, onSuccess }: UseExportActionsProps) {
    const { t } = useTranslation();
    const alert = useDialogStore((state) => state.alert);
    const showError = useDialogStore((state) => state.showError);
    
    const segments = useTranscriptSessionStore((state) => state.segments);
    const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
    const historyItems = useHistoryStore((state) => state.items);
    const activeProject = useProjectStore((state) => state.projects.find((item) => item.id === state.activeProjectId) || null);
    
    const [fileName, setFileName] = useState('');
    const [directory, setDirectory] = useState(localStorage.getItem('sona_last_export_dir') || '');
    const [exportFormat, setExportFormat] = useState<ExportFormat>('srt');
    const [exportMode, setExportMode] = useState<ExportMode>('original');
    const [isExporting, setIsExporting] = useState(false);

    const hasTranslation = segments.some(seg => typeof seg.translation === 'string' && seg.translation.trim().length > 0);
    
    const defaultFileName = useMemo(() => {
        const historyItem = historyItems.find(item => item.id === sourceHistoryId);
        const prefix = (activeProject?.defaults.exportFileNamePrefix || '').trim();
        const sanitizedPrefix = prefix.replace(/[\\/:*?"<>|]/g, '_').trim();

        if (historyItem) {
            const sanitized = historyItem.title.replace(/[\\/:*?"<>|]/g, '_');
            return sanitizedPrefix ? `${sanitizedPrefix} ${sanitized}`.trim() : sanitized;
        }

        return sanitizedPrefix;
    }, [activeProject?.defaults.exportFileNamePrefix, historyItems, sourceHistoryId]);

    // Initial value for filename from history title
    useEffect(() => {
        if (!isOpen) {
            return;
        }

        queueMicrotask(() => {
            setFileName(defaultFileName);
        });
    }, [defaultFileName, isOpen]);

    useEffect(() => {
        if (!isOpen || hasTranslation || exportMode === 'original') {
            return;
        }

        queueMicrotask(() => {
            setExportMode('original');
        });
    }, [exportMode, hasTranslation, isOpen]);

    const handleBrowse = async () => {
        try {
            const selected = await openDialog({
                directory: true,
                multiple: false,
                defaultPath: directory || undefined,
            });
            if (selected && typeof selected === 'string') {
                setDirectory(selected);
                localStorage.setItem('sona_last_export_dir', selected);
            }
        } catch (error) {
            logger.error('Failed to open directory picker:', error);
        }
    };

    const handleExport = async () => {
        if (!fileName.trim()) {
            await alert(t('export.invalid_filename'), { variant: 'warning' });
            return;
        }
        if (!directory) {
            await alert(t('export.select_directory'), { variant: 'warning' });
            return;
        }

        setIsExporting(true);
        try {
            await exportTranscriptToDirectory({
                segments,
                directory,
                baseFileName: fileName,
                format: exportFormat,
                mode: exportMode,
            });
            
            await alert(t('export.success'), { variant: 'success' });
            onSuccess();
        } catch (error) {
            await showError({
                code: 'export.failed',
                messageKey: 'errors.export.failed',
                cause: error,
            });
        } finally {
            setIsExporting(false);
        }
    };

    return {
        fileName,
        setFileName,
        directory,
        exportFormat,
        setExportFormat,
        exportMode,
        setExportMode,
        isExporting,
        hasTranslation,
        handleBrowse,
        handleExport
    };
}
