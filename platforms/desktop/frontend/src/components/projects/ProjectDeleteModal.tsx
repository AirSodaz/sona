import { AlertTriangle } from 'lucide-react';
import type React from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ProjectRecord } from '../../types/project';
import { Modal } from '../Modal';

interface ProjectDeleteModalProps {
  isOpen: boolean;
  project: ProjectRecord | null;
  itemCount: number;
  onClose: () => void;
  onConfirm: (cascadeAction: 'moveToInbox' | 'deleteItems') => Promise<void>;
}

export function ProjectDeleteModal({
  isOpen,
  project,
  itemCount,
  onClose,
  onConfirm,
}: ProjectDeleteModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [cascadeAction, setCascadeAction] = useState<'moveToInbox' | 'deleteItems'>('moveToInbox');
  const [isDeleting, setIsDeleting] = useState(false);

  if (!isOpen || !project) {
    return null;
  }

  const handleDelete = async () => {
    setIsDeleting(true);
    try {
      await onConfirm(cascadeAction);
      onClose();
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: 'var(--color-error)' }}>
          <AlertTriangle size={20} />
          <span>{t('projects.delete_project_title', { defaultValue: 'Delete Project' })}</span>
        </div>
      }
      size="md"
      footer={
        <div
          style={{
            display: 'flex',
            gap: 'var(--spacing-sm)',
            justifyContent: 'flex-end',
            width: '100%',
          }}
        >
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onClose}
            disabled={isDeleting}
          >
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button
            type="button"
            className="btn btn-danger"
            onClick={() => void handleDelete()}
            disabled={isDeleting}
          >
            {isDeleting
              ? t('common.deleting', { defaultValue: 'Deleting...' })
              : t('common.delete', { defaultValue: 'Delete' })}
          </button>
        </div>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        <p style={{ margin: 0, fontSize: 13, color: 'var(--color-text-primary)' }}>
          {t('projects.delete_project_confirm_msg', {
            name: project.name,
            defaultValue: 'Are you sure you want to delete "{{name}}"?',
          })}
        </p>

        {itemCount > 0 ? (
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: 10,
              padding: 12,
              background: 'var(--color-bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--color-border)',
            }}
          >
            <span style={{ fontSize: 12, fontWeight: 500, color: 'var(--color-text-secondary)' }}>
              {t('projects.delete_items_handling', {
                count: itemCount,
                defaultValue: 'This project contains {{count}} items. Choose how to handle them:',
              })}
            </span>

            <label
              style={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: 8,
                cursor: 'pointer',
                fontSize: 13,
              }}
            >
              <input
                type="radio"
                name="cascadeAction"
                value="moveToInbox"
                checked={cascadeAction === 'moveToInbox'}
                onChange={() => setCascadeAction('moveToInbox')}
                style={{ marginTop: 3 }}
              />
              <div>
                <strong>
                  {t('projects.move_to_inbox_option', {
                    defaultValue: 'Move to Inbox (Recommended)',
                  })}
                </strong>
                <p style={{ margin: '2px 0 0 0', fontSize: 12, color: 'var(--color-text-muted)' }}>
                  {t('projects.move_to_inbox_desc', {
                    defaultValue:
                      'Keep all recordings and transcriptions; reset their project to Inbox.',
                  })}
                </p>
              </div>
            </label>

            <label
              style={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: 8,
                cursor: 'pointer',
                fontSize: 13,
              }}
            >
              <input
                type="radio"
                name="cascadeAction"
                value="deleteItems"
                checked={cascadeAction === 'deleteItems'}
                onChange={() => setCascadeAction('deleteItems')}
                style={{ marginTop: 3 }}
              />
              <div>
                <strong style={{ color: 'var(--color-error)' }}>
                  {t('projects.move_to_trash_option', { defaultValue: 'Move all items to Trash' })}
                </strong>
                <p style={{ margin: '2px 0 0 0', fontSize: 12, color: 'var(--color-text-muted)' }}>
                  {t('projects.move_to_trash_desc', {
                    defaultValue: 'Soft-delete all associated items and send them to Trash.',
                  })}
                </p>
              </div>
            </label>
          </div>
        ) : (
          <p style={{ margin: 0, fontSize: 12, color: 'var(--color-text-muted)' }}>
            {t('projects.delete_empty_project_note', {
              defaultValue: 'This project has no records and will be removed immediately.',
            })}
          </p>
        )}
      </div>
    </Modal>
  );
}
