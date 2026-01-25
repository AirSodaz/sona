import React, { useState, useRef } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { saveTranscript } from '../utils/fileExport';
import { ExportFormat } from '../utils/exportFormats';

// Icons
const DownloadIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
        <polyline points="7,10 12,15 17,10" />
        <line x1="12" x2="12" y1="15" y2="3" />
    </svg>
);

const ChevronDownIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polyline points="6,9 12,15 18,9" />
    </svg>
);

const FileTextIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
        <polyline points="14,2 14,8 20,8" />
        <line x1="16" x2="8" y1="13" y2="13" />
        <line x1="16" x2="8" y1="17" y2="17" />
        <polyline points="10,9 9,9 8,9" />
    </svg>
);

const CodeIcon = () => (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polyline points="16,18 22,12 16,6" />
        <polyline points="8,6 2,12 8,18" />
    </svg>
);

interface ExportButtonProps {
    className?: string;
}

export const ExportButton: React.FC<ExportButtonProps> = ({ className = '' }) => {
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
            alert('No segments to export');
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
            alert('Failed to export transcript');
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
            <button
                className="btn btn-secondary"
                onClick={() => setIsOpen(!isOpen)}
                disabled={isExporting || segments.length === 0}
            >
                <DownloadIcon />
                <span>{isExporting ? 'Exporting...' : 'Export'}</span>
                <ChevronDownIcon />
            </button>

            {isOpen && (
                <div className="export-dropdown">
                    {exportOptions.map((option) => (
                        <button
                            key={option.format}
                            className="export-dropdown-item"
                            onClick={() => handleExport(option.format)}
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
