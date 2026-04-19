import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { join } from '@tauri-apps/api/path';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { useDialogStore } from '../stores/dialogStore';
import { exportSegments, getFileExtension, ExportFormat, ExportMode } from '../utils/exportFormats';
import { exportToPath } from '../utils/fileExport';
import { Dropdown } from './Dropdown';
import { XIcon, FolderIcon } from './Icons';

interface ExportModalProps {
    isOpen: boolean;
    onClose: () => void;
}

/**
 * Modal for exporting transcript segments with customizable options.
 */
export function ExportModal({ isOpen, onClose }: ExportModalProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const alert = useDialogStore((state) => state.alert);
    const showError = useDialogStore((state) => state.showError);
    
    const segments = useTranscriptStore((state) => state.segments);
    const sourceHistoryId = useTranscriptStore((state) => state.sourceHistoryId);
    const historyItems = useHistoryStore((state) => state.items);
    
    const [fileName, setFileName] = useState('');
    const [directory, setDirectory] = useState(localStorage.getItem('sona_last_export_dir') || '');
    const [exportFormat, setExportFormat] = useState<ExportFormat>('srt');
    const [exportMode, setExportMode] = useState<ExportMode>('original');
    const [isExporting, setIsExporting] = useState(false);

    const hasTranslation = segments.some(seg => typeof seg.translation === 'string' && seg.translation.trim().length > 0);

    // Initial value for filename from history title
    useEffect(() => {
        if (isOpen) {
            const historyItem = historyItems.find(item => item.id === sourceHistoryId);
            if (historyItem) {
                // Sanitize filename: remove characters that are usually illegal in filenames
                const sanitized = historyItem.title.replace(/[\\/:*?"<>|]/g, '_');
                setFileName(sanitized);
            }
            
            // Reset export mode if no translations
            if (!hasTranslation && exportMode !== 'original') {
                setExportMode('original');
            }
        }
    }, [isOpen, sourceHistoryId, historyItems, hasTranslation, exportMode]);

    // Keyboard support (Escape to close)
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (!isOpen) return;
            if (e.key === 'Escape') {
                e.preventDefault();
                onClose();
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [isOpen, onClose]);

    const handleBrowse = async () => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                defaultPath: directory || undefined,
            });
            if (selected && typeof selected === 'string') {
                setDirectory(selected);
                localStorage.setItem('sona_last_export_dir', selected);
            }
        } catch (error) {
            console.error('Failed to open directory picker:', error);
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
            const content = exportSegments(segments, exportFormat, exportMode);
            const extension = getFileExtension(exportFormat);
            const fullPath = await join(directory, `${fileName}${extension}`);
            
            await exportToPath(content, fullPath);
            
            await alert(t('export.success'), { variant: 'success' });
            onClose();
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

    if (!isOpen) return null;

    const exportOptions = [
        { value: 'srt', label: 'SubRip (.srt)' },
        { value: 'vtt', label: 'WebVTT (.vtt)' },
        { value: 'json', label: 'JSON (.json)' },
        { value: 'txt', label: 'Plain Text (.txt)' },
    ];

    return (
        <div className="settings-overlay" onClick={onClose} style={{ zIndex: 2000 }}>
            <div
                className="dialog-modal"
                onClick={(e) => e.stopPropagation()}
                role="dialog"
                aria-modal="true"
                aria-labelledby="export-modal-title"
                style={{
                    background: 'var(--color-bg-elevated)',
                    borderRadius: 'var(--radius-lg)',
                    boxShadow: 'var(--shadow-xl)',
                    width: '500px',
                    maxWidth: '95vw',
                    padding: 'var(--spacing-lg)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 'var(--spacing-md)',
                    border: '1px solid var(--color-border)',
                }}
            >
                {/* Header */}
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <h3 id="export-modal-title" style={{ fontSize: '1.125rem', fontWeight: 600, color: 'var(--color-text-primary)', margin: 0 }}>
                        {t('export.modal_title')}
                    </h3>
                    <button className="btn btn-icon" onClick={onClose} aria-label={t('common.close')}>
                        <XIcon />
                    </button>
                </div>

                {/* Content */}
                <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-lg)' }}>
                    {/* Filename */}
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                        <label style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                            {t('export.filename')}
                        </label>
                        <input
                            type="text"
                            value={fileName}
                            onChange={(e) => setFileName(e.target.value)}
                            style={{
                                padding: '8px 12px',
                                borderRadius: '4px',
                                border: '1px solid var(--color-border)',
                                background: 'var(--color-bg-input)',
                                color: 'var(--color-text-primary)',
                                outline: 'none'
                            }}
                            placeholder={t('export.filename')}
                        />
                    </div>

                    {/* Directory */}
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                        <label style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                            {t('export.directory')}
                        </label>
                        <div style={{ display: 'flex', gap: '8px' }}>
                            <input
                                type="text"
                                value={directory}
                                readOnly
                                style={{
                                    flex: 1,
                                    padding: '8px 12px',
                                    borderRadius: '4px',
                                    border: '1px solid var(--color-border)',
                                    background: 'var(--color-bg-subtle)',
                                    color: 'var(--color-text-secondary)',
                                    fontSize: '0.875rem',
                                    cursor: 'default'
                                }}
                            />
                            <button className="btn" onClick={handleBrowse} style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                                <FolderIcon width={16} height={16} />
                                {t('settings.browse')}
                            </button>
                        </div>
                    </div>

                    <div style={{ display: 'flex', gap: 'var(--spacing-lg)' }}>
                        {/* Format */}
                        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                            <label style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                                {t('export.format')}
                            </label>
                            <Dropdown
                                value={exportFormat}
                                onChange={(val) => setExportFormat(val as ExportFormat)}
                                options={exportOptions}
                                style={{ width: '100%' }}
                            />
                        </div>

                        {/* Mode Selection */}
                        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 'var(--spacing-xs)' }}>
                            <label style={{ fontWeight: 500, color: 'var(--color-text-primary)', fontSize: '0.875rem' }}>
                                {t('panel.mode_selection')}
                            </label>
                            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', padding: '4px 0' }}>
                                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer', fontSize: '0.875rem' }}>
                                    <input
                                        type="radio"
                                        name="exportMode"
                                        checked={exportMode === 'original'}
                                        onChange={() => setExportMode('original')}
                                    />
                                    {t('export.mode_original')}
                                </label>
                                <label style={{
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: '8px',
                                    cursor: hasTranslation ? 'pointer' : 'not-allowed',
                                    fontSize: '0.875rem',
                                    opacity: hasTranslation ? 1 : 0.5
                                }}>
                                    <input
                                        type="radio"
                                        name="exportMode"
                                        checked={exportMode === 'translation'}
                                        onChange={() => hasTranslation && setExportMode('translation')}
                                        disabled={!hasTranslation}
                                    />
                                    {t('export.mode_translation')}
                                </label>
                                <label style={{
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: '8px',
                                    cursor: hasTranslation ? 'pointer' : 'not-allowed',
                                    fontSize: '0.875rem',
                                    opacity: hasTranslation ? 1 : 0.5
                                }}>
                                    <input
                                        type="radio"
                                        name="exportMode"
                                        checked={exportMode === 'bilingual'}
                                        onChange={() => hasTranslation && setExportMode('bilingual')}
                                        disabled={!hasTranslation}
                                    />
                                    {t('export.mode_bilingual')}
                                </label>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Footer */}
                <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 'var(--spacing-sm)', marginTop: 'var(--spacing-md)' }}>
                    <button className="btn" onClick={onClose} disabled={isExporting}>
                        {t('common.cancel')}
                    </button>
                    <button className="btn btn-primary" onClick={handleExport} disabled={isExporting}>
                        {isExporting ? t('export.exporting') : t('export.button')}
                    </button>
                </div>
            </div>
        </div>
    );
}
