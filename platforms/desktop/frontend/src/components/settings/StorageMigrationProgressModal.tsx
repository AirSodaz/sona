import { AlertTriangle, CheckCircle2, FolderSync } from 'lucide-react';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '../Modal';

export interface StorageMigrationProgressPayload {
  phase: string;
  currentFile: string;
  copiedBytes: number;
  totalBytes: number;
  percent: number;
}

interface StorageMigrationProgressModalProps {
  isOpen: boolean;
  title: string;
  targetPath: string;
  progress: StorageMigrationProgressPayload | null;
  isCompleted: boolean;
  errorMessage?: string | null;
}

function formatSize(bytes: number): string {
  if (bytes <= 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), sizes.length - 1);
  return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

export const StorageMigrationProgressModal: React.FC<StorageMigrationProgressModalProps> = ({
  isOpen,
  title,
  targetPath,
  progress,
  isCompleted,
  errorMessage,
}) => {
  const { t } = useTranslation();

  React.useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Tab' || e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
      }
    };
    window.addEventListener('keydown', handleKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', handleKeyDown, { capture: true });
  }, [isOpen]);

  if (!isOpen) return null;

  const percent = isCompleted ? 100 : Math.max(0, Math.min(100, progress?.percent ?? 0));
  const copiedBytes = progress?.copiedBytes ?? 0;
  const totalBytes = progress?.totalBytes ?? 0;

  const modalTitle = (
    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
      {isCompleted ? (
        <CheckCircle2 size={20} color="var(--color-success, #4ea067)" />
      ) : errorMessage ? (
        <AlertTriangle size={20} color="var(--color-error, #e03e3e)" />
      ) : (
        <FolderSync size={20} color="var(--color-info, #0b6e99)" />
      )}
      <span style={{ fontSize: '1rem', fontWeight: 600 }}>{title}</span>
    </div>
  );

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => undefined}
      title={modalTitle}
      size="sm"
      hideCloseButton={true}
      closeOnOverlayClick={false}
      closeOnEsc={false}
      bodyStyle={{ padding: '16px 20px 20px' }}
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
        <div
          style={{
            fontSize: '0.8rem',
            color: 'var(--color-text-secondary)',
            wordBreak: 'break-all',
            backgroundColor: 'var(--color-bg-secondary)',
            padding: '8px 10px',
            borderRadius: 'var(--radius-sm, 6px)',
            border: '1px solid var(--color-border)',
          }}
        >
          {targetPath}
        </div>

        {errorMessage ? (
          <div
            style={{
              padding: '12px',
              backgroundColor: 'color-mix(in srgb, var(--color-error, #e03e3e) 10%, transparent)',
              border:
                '1px solid color-mix(in srgb, var(--color-error, #e03e3e) 24%, var(--color-border))',
              borderRadius: 'var(--radius-sm, 6px)',
              fontSize: '0.85rem',
              color: 'var(--color-error, #e03e3e)',
            }}
          >
            {errorMessage}
          </div>
        ) : (
          <>
            <div>
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  fontSize: '0.8rem',
                  marginBottom: '6px',
                  color: 'var(--color-text-secondary, #9ca3af)',
                }}
              >
                <span>
                  {isCompleted
                    ? t('settings.storage.migration_completed', {
                        defaultValue: 'Migration completed',
                      })
                    : progress?.phase === 'copying'
                      ? t('settings.storage.migration_copying_files', {
                          defaultValue: 'Copying files...',
                        })
                      : t('settings.storage.migration_preparing', {
                          defaultValue: 'Preparing migration...',
                        })}
                </span>
                <span>
                  {percent.toFixed(1)}%
                  {totalBytes > 0 && ` (${formatSize(copiedBytes)} / ${formatSize(totalBytes)})`}
                </span>
              </div>
              <div
                style={{
                  width: '100%',
                  height: '6px',
                  backgroundColor: 'var(--color-bg-tertiary)',
                  borderRadius: 'var(--radius-full, 9999px)',
                  overflow: 'hidden',
                }}
              >
                <div
                  style={{
                    height: '100%',
                    width: `${percent}%`,
                    backgroundColor: isCompleted
                      ? 'var(--color-success, #4ea067)'
                      : 'var(--color-info, #0b6e99)',
                    borderRadius: 'var(--radius-full, 9999px)',
                    transition: 'width 0.2s ease',
                  }}
                />
              </div>
            </div>

            {progress?.currentFile && !isCompleted && (
              <div
                style={{
                  fontSize: '0.75rem',
                  color: 'var(--color-text-muted)',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}
              >
                {t('common.processing', { defaultValue: 'Processing' })}: {progress.currentFile}
              </div>
            )}

            <div
              style={{
                fontSize: '0.78rem',
                color: isCompleted
                  ? 'var(--color-success, #4ea067)'
                  : 'var(--color-text-secondary)',
                lineHeight: 1.4,
              }}
            >
              {isCompleted
                ? t('settings.storage.migration_completed_hint', {
                    defaultValue: 'New storage configuration has taken effect. Refreshing views...',
                  })
                : t('settings.storage.migration_in_progress_hint', {
                    defaultValue:
                      'Transferring files and reloading database connections, please wait...',
                  })}
            </div>
          </>
        )}
      </div>
    </Modal>
  );
};
