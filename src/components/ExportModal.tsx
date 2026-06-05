import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ExportFormat } from '../utils/exportFormats';
import { Dropdown } from './Dropdown';
import { FolderIcon, CopyIcon, CheckIcon } from './Icons';
import { Modal } from './Modal';
import { useExportActions } from '../hooks/useExportActions';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { logger } from '../utils/logger';
import type { TranscriptSegment } from '../types/transcript';

function decodeHtmlEntities(html: string): string {
    return html
        .replace(/&nbsp;/g, ' ')
        .replace(/&amp;/g, '&')
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&quot;/g, '"')
        .replace(/&#39;/g, "'");
}

function htmlToPlainText(html: string): string {
    if (!html) return '';
    let text = html.replace(/<br\s*\/?>/gi, '\n');
    text = text.replace(/<\/(p|div)>/gi, '\n');
    text = text.replace(/<\/?[^>]+(>|$)/g, '');
    return decodeHtmlEntities(text).trim();
}

function formatSegmentsToPlainText(
    segments: TranscriptSegment[],
    mode: 'original' | 'translation' | 'bilingual'
): string {
    return segments
        .filter((seg) => seg.isFinal)
        .map((seg) => {
            const speakerLabel = seg.speaker?.label?.trim();
            const originalText = htmlToPlainText(seg.text || '');
            const translationText = htmlToPlainText(seg.translation || '');

            let text: string;
            if (mode === 'translation') {
                text = translationText;
            } else if (mode === 'bilingual') {
                if (originalText && translationText) {
                    text = `${originalText}\n${translationText}`;
                } else {
                    text = originalText || translationText;
                }
            } else {
                text = originalText;
            }

            if (!text) return '';

            if (speakerLabel) {
                return `${speakerLabel}: ${text}`;
            }
            return text;
        })
        .filter((text) => text.length > 0)
        .join('\n\n');
}

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

    const segments = useTranscriptSessionStore((state) => state.segments);

    const [copied, setCopied] = React.useState(false);
    const timeoutRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);

    React.useEffect(() => {
        return () => {
            if (timeoutRef.current) {
                clearTimeout(timeoutRef.current);
            }
        };
    }, []);

    const handleCopy = async () => {
        try {
            const plainText = formatSegmentsToPlainText(segments, exportMode);
            await navigator.clipboard.writeText(plainText);

            if (timeoutRef.current) {
                clearTimeout(timeoutRef.current);
            }
            setCopied(true);
            timeoutRef.current = setTimeout(() => {
                setCopied(false);
            }, 2000);
        } catch (error) {
            logger.error('Failed to copy transcript to clipboard:', error);
        }
    };

    if (!isOpen) return null;

    const exportOptions = [
        { value: 'srt', label: 'SubRip (.srt)' },
        { value: 'vtt', label: 'WebVTT (.vtt)' },
        { value: 'json', label: 'JSON (.json)' },
        { value: 'txt', label: 'Plain Text (.txt)' },
        { value: 'md', label: 'Markdown (.md)' },
    ];

    return (
        <Modal
            isOpen={isOpen}
            onClose={onClose}
            title={t('export.modal_title')}
            size="md"
            footer={
                <div style={{ display: 'flex', width: '100%', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginRight: 'auto' }}>
                        <button
                            className="btn btn-secondary"
                            onClick={handleCopy}
                            style={{ display: 'flex', alignItems: 'center', gap: '4px' }}
                        >
                            <CopyIcon width={16} height={16} />
                            {t('export.copy_to_clipboard')}
                        </button>
                        {copied && (
                            <CheckIcon
                                data-testid="copy-success-check"
                                width={16}
                                height={16}
                                style={{ color: 'var(--color-success)', animation: 'fade-in 0.2s ease-in-out' }}
                            />
                        )}
                    </div>
                    <div style={{ display: 'flex', gap: '8px' }}>
                        <button className="btn btn-secondary" onClick={onClose} disabled={isExporting}>
                            {t('common.cancel')}
                        </button>
                        <button className="btn btn-primary" onClick={handleExport} disabled={isExporting}>
                            {isExporting ? t('export.exporting') : t('export.button')}
                        </button>
                    </div>
                </div>
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
