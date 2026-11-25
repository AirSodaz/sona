import type React from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { ExportModal } from './ExportModal';
import { DownloadIcon } from './Icons';
import { IconButton } from './ui/Button';

/** Props for ExportButton. */
interface ExportButtonProps {
  /** Optional CSS class name. */
  className?: string;
}

/**
 * Button component for opening the export modal.
 *
 * @param props - Component props.
 * @return The export button component.
 */
export function ExportButton({ className = '' }: ExportButtonProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const segments = useTranscriptSessionStore((state) => state.segments);

  // Only show if there's transcript content
  if (segments.length === 0) {
    return null;
  }

  return (
    <>
      <div className={`export-menu ${className}`} style={{ display: 'inline-block' }}>
        <IconButton
          id="export-menu-button"
          tier="ghost"
          size="md"
          icon={<DownloadIcon />}
          onClick={() => setIsModalOpen(true)}
          disabled={segments.length === 0}
          aria-label={t('export.button')}
          tooltipPos="bottom"
        />
      </div>

      <ExportModal isOpen={isModalOpen} onClose={() => setIsModalOpen(false)} />
    </>
  );
}
