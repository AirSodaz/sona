import React, { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { saveTranscript } from '../utils/fileExport';
import { ExportFormat } from '../utils/exportFormats';
import { DownloadIcon, ChevronDownIcon, FileTextIcon, CodeIcon } from './Icons';



/** Props for ExportButton. */
interface ExportButtonProps {
    /** Optional CSS class name. */
    className?: string;
}

/**
 * Dropdown button component for exporting transcript segments in various formats.
 *
 * @param props - Component props.
 * @return The export button component.
 */
export function ExportButton({ className = '' }: ExportButtonProps): React.JSX.Element {
    const { t } = useTranslation();
    const { alert } = useDialogStore();
    const [isOpen, setIsOpen] = useState(false);
    const [isExporting, setIsExporting] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);

    const segments = useTranscriptStore((state) => state.segments);

    // Close dropdown when clicking outside
    React.useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };

        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const handleExport = async (format: ExportFormat) => {
        if (segments.length === 0) {
            alert(t('export.no_segments'), { variant: 'info' });
            return;
        }

        setIsExporting(true);
        setIsOpen(false);

        try {
            await saveTranscript({
                segments,
                format,
                defaultFileName: `transcript_${new Date().toISOString().slice(0, 10)}`,
            });
        } catch (error) {
            console.error('Export failed:', error);
            alert(t('export.failed'), { variant: 'error' });
        } finally {
            setIsExporting(false);
        }
    };

    const exportOptions: { format: ExportFormat; label: string; icon: React.ReactNode }[] = [
        { format: 'srt', label: 'SubRip (.srt)', icon: <FileTextIcon /> },
        { format: 'vtt', label: 'WebVTT (.vtt)', icon: <FileTextIcon /> },
        { format: 'json', label: 'JSON (.json)', icon: <CodeIcon /> },
        { format: 'txt', label: 'Plain Text (.txt)', icon: <FileTextIcon /> },
    ];

    return (
        <div className={`export-menu ${className}`} ref={dropdownRef}>
            <div
                data-tooltip={segments.length === 0 ? t('export.no_segments') : undefined}
                data-tooltip-pos="bottom"
                style={{ display: 'inline-block' }}
            >
                <button
                    id="export-menu-button"
                    className="btn btn-secondary"
                    onClick={() => setIsOpen(!isOpen)}
                    disabled={isExporting || segments.length === 0}
                    style={isExporting || segments.length === 0 ? { pointerEvents: 'none' } : undefined}
                    aria-haspopup="true"
                    aria-expanded={isOpen}
                    aria-controls="export-menu-dropdown"
                >
                    <DownloadIcon />
                    <span>{isExporting ? t('export.exporting') : t('export.button')}</span>
                    <ChevronDownIcon />
                </button>
            </div>

            {isOpen && (
                <div
                    id="export-menu-dropdown"
                    className="export-dropdown"
                    role="menu"
                    aria-labelledby="export-menu-button"
                >
                    {exportOptions.map((option) => (
                        <button
                            key={option.format}
                            className="export-dropdown-item"
                            onClick={() => handleExport(option.format)}
                            role="menuitem"
                        >
                            {option.icon}
                            <span>{option.label}</span>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
};

export default ExportButton;
