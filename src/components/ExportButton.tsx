import React, { useState, useRef, useEffect } from 'react';
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
    const [position, setPosition] = useState<'bottom' | 'top'>('bottom');
    const [isExporting, setIsExporting] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    const segments = useTranscriptStore((state) => state.segments);

    // Close dropdown when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };

        if (isOpen && dropdownRef.current) {
            const rect = dropdownRef.current.getBoundingClientRect();
            // Estimate dropdown height
            const estimatedHeight = 200;
            const spaceBelow = window.innerHeight - rect.bottom;

            if (spaceBelow < estimatedHeight && rect.top > estimatedHeight) {
                setPosition('top');
            } else {
                setPosition('bottom');
            }
        }

        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [isOpen]);

    // Focus management when opening
    useEffect(() => {
        if (isOpen && menuRef.current) {
            const firstButton = menuRef.current.querySelector('button');
            if (firstButton) {
                requestAnimationFrame(() => firstButton.focus());
            }
        }
    }, [isOpen]);

    const handleBlur = (e: React.FocusEvent) => {
        // Close menu if focus leaves the component
        if (!dropdownRef.current?.contains(e.relatedTarget as Node)) {
            setIsOpen(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (!isOpen) return;

        if (e.key === 'Escape') {
            e.preventDefault();
            setIsOpen(false);
            triggerRef.current?.focus();
            return;
        }

        if (menuRef.current) {
            const buttons = Array.from(menuRef.current.querySelectorAll('button'));
            const currentIndex = buttons.indexOf(document.activeElement as HTMLButtonElement);

            if (e.key === 'ArrowDown') {
                e.preventDefault();
                const nextIndex = (currentIndex + 1) % buttons.length;
                buttons[nextIndex].focus();
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                const prevIndex = (currentIndex - 1 + buttons.length) % buttons.length;
                buttons[prevIndex].focus();
            } else if (e.key === 'Home') {
                e.preventDefault();
                buttons[0].focus();
            } else if (e.key === 'End') {
                e.preventDefault();
                buttons[buttons.length - 1].focus();
            }
        }
    };

    const handleExport = async (format: ExportFormat) => {
        if (segments.length === 0) {
            await alert(t('export.no_segments'), {variant: 'info'});
            return;
        }

        setIsExporting(true);
        setIsOpen(false);
        // Restore focus to trigger button after selection
        triggerRef.current?.focus();

        try {
            await saveTranscript({
                segments,
                format,
                defaultFileName: `transcript_${new Date().toISOString().slice(0, 10)}`,
            });
        } catch (error) {
            console.error('Export failed:', error);
            await alert(t('export.failed'), {variant: 'error'});
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
        <div
            className={`export-menu ${className}`}
            ref={dropdownRef}
            onKeyDown={handleKeyDown}
            onBlur={handleBlur}
        >
            <div
                data-tooltip={segments.length === 0 ? t('export.no_segments') : undefined}
                data-tooltip-pos="bottom"
                style={{ display: 'inline-block' }}
            >
                <button
                    ref={triggerRef}
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
                    ref={menuRef}
                    id="export-menu-dropdown"
                    className={`export-dropdown position-${position}`}
                    role="menu"
                    aria-labelledby="export-menu-button"
                >
                    {exportOptions.map((option) => (
                        <button
                            type="button"
                            key={option.format}
                            className="export-dropdown-item"
                            onClick={() => handleExport(option.format)}
                            role="menuitem"
                            tabIndex={-1}
                        >
                            {option.icon}
                            <span>{option.label}</span>
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}
