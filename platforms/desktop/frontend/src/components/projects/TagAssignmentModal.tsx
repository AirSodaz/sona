import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { HistoryItem } from '../../types/history';
import type { ProjectRecord } from '../../types/project';
import { Checkbox } from '../Checkbox';
import { Modal } from '../Modal';

interface TagAssignmentModalProps {
  isOpen: boolean;
  items: HistoryItem[];
  tags: ProjectRecord[];
  onClose: () => void;
  onApply: (addTagIds: string[], removeTagIds: string[]) => Promise<void>;
}

function getItemTagIds(item: HistoryItem): string[] {
  return item.tagIds ?? (item.projectId ? [item.projectId] : []);
}

export function TagAssignmentModal({
  isOpen,
  items,
  tags,
  onClose,
  onApply,
}: TagAssignmentModalProps): React.JSX.Element | null {
  if (!isOpen) {
    return null;
  }

  const assignmentKey = JSON.stringify(items.map((item) => [item.id, getItemTagIds(item)]));
  return (
    <OpenTagAssignmentModal
      key={assignmentKey}
      items={items}
      tags={tags}
      onClose={onClose}
      onApply={onApply}
    />
  );
}

function OpenTagAssignmentModal({
  items,
  tags,
  onClose,
  onApply,
}: Omit<TagAssignmentModalProps, 'isOpen'>): React.JSX.Element {
  const { t } = useTranslation();
  const [overrides, setOverrides] = useState<Record<string, boolean>>({});
  const [isSaving, setIsSaving] = useState(false);
  const counts = useMemo(() => {
    const next = new Map<string, number>();
    items.forEach((item) => {
      new Set(getItemTagIds(item)).forEach((tagId) => {
        next.set(tagId, (next.get(tagId) || 0) + 1);
      });
    });
    return next;
  }, [items]);

  const handleApply = async () => {
    const addTagIds: string[] = [];
    const removeTagIds: string[] = [];
    Object.entries(overrides).forEach(([tagId, enabled]) => {
      const count = counts.get(tagId) || 0;
      if (enabled && count < items.length) addTagIds.push(tagId);
      if (!enabled && count > 0) removeTagIds.push(tagId);
    });

    setIsSaving(true);
    try {
      await onApply(addTagIds, removeTagIds);
      onClose();
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Modal
      isOpen
      onClose={onClose}
      title={t('projects.edit_tags', { defaultValue: 'Edit Tags' })}
      size="sm"
      footer={
        <>
          <button type="button" className="btn btn-secondary" onClick={onClose} disabled={isSaving}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </button>
          <button type="button" className="btn btn-primary" onClick={() => void handleApply()} disabled={isSaving}>
            {t('common.save', { defaultValue: 'Save' })}
          </button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
        {tags.length === 0 ? (
          <p>{t('projects.no_tags', { defaultValue: 'No tags yet.' })}</p>
        ) : tags.map((tag) => {
          const count = counts.get(tag.id) || 0;
          const initialChecked = count === items.length && items.length > 0;
          const initialMixed = count > 0 && count < items.length;
          const hasOverride = Object.prototype.hasOwnProperty.call(overrides, tag.id);
          const checked = hasOverride ? overrides[tag.id] : initialChecked;
          const indeterminate = !hasOverride && initialMixed;
          return (
            <div key={tag.id} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span
                aria-hidden="true"
                style={{ width: 10, height: 10, borderRadius: '50%', background: tag.color || '#64748b' }}
              />
              <Checkbox
                checked={checked}
                indeterminate={indeterminate}
                label={tag.name}
                onChange={(next) => setOverrides((current) => ({ ...current, [tag.id]: next }))}
              />
            </div>
          );
        })}
      </div>
    </Modal>
  );
}
