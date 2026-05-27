import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ExportFormat } from '../utils/exportFormats';
import { Dropdown } from './Dropdown';
import { FolderIcon } from './Icons';
import { Modal } from './Modal';
import { useExportActions } from '../hooks/useExportActions';

interface ExportModalProps {
    isOpen: boolean;
    onClose: () => void;
}

/**
 * Modal for exporting transcript segments with customizable options.
 */
export function ExportModal({ isOpen, onClose }: ExportModalProps): React.JSX.Element | null {
    const { t } = useTranslation();
    
    const {
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
    } = useExportActions({ isOpen, onSuccess: onClose });

    if (!isOpen) return null;

    const exportOptions = [
        { value: 'srt', label: 'SubRip (.srt)' },
        { value: 'vtt', label: 'WebVTT (.vtt)' },
        { value: 'json', label: 'JSON (.json)' },
        { value: 'txt', label: 'Plain Text (.txt)' },
    ];

    return (
        <Modal
            isOpen={isOpen}
            onClose={onClose}
            title={t('export.modal_title')}
            size="md"
            footer={
                <>
                    <button className="btn btn-secondary" onClick={onClose} disabled={isExporting}>
                        {t('common.cancel')}
                    </button>
                    <button className="btn btn-primary" onClick={handleExport} disabled={isExporting}>
                        {isExporting ? t('export.exporting') : t('export.button')}
                    </button>
                </>
            }
        >
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
                            borderRadius: 'var(--radius-sm)',
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
                                borderRadius: 'var(--radius-sm)',
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
        </Modal>
    );
}
