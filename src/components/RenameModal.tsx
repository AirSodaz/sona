import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { SparklesIcon, MicIcon, FileTextIcon } from './Icons';
import { IconPicker } from './IconPicker';
import { Modal } from './Modal';
import { FormField } from './FormField';

interface RenameModalProps {
  isOpen: boolean;
  onClose: () => void;
  initialTitle: string;
  initialIcon?: string;
  defaultType?: 'recording' | 'batch';
  onRename: (title: string, icon?: string) => void;
  onAiAction?: () => Promise<string>;
}

export function RenameModal({
  isOpen,
  onClose,
  initialTitle,
  initialIcon,
  defaultType,
  onRename,
  onAiAction,
}: RenameModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const [title, setTitle] = useState(initialTitle);
  const [icon, setIcon] = useState(initialIcon || '');
  const [isAiLoading, setIsAiLoading] = useState(false);
  const [prevIsOpen, setPrevIsOpen] = useState(isOpen);
  const inputRef = useRef<HTMLInputElement>(null);

  // Sync title and icon when modal opens
  if (isOpen && !prevIsOpen) {
    setPrevIsOpen(true);
    setTitle(initialTitle);
    setIcon(initialIcon || '');
  } else if (!isOpen && prevIsOpen) {
    setPrevIsOpen(false);
  }

  useEffect(() => {
    if (!isOpen) return;

    // Focus input on mount after modal transition
    const focusTimer = setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 100);

    return () => clearTimeout(focusTimer);
  }, [isOpen]);

  if (!isOpen) return null;

  const handleSave = () => {
    if (title.trim()) {
      onRename(title.trim(), icon || undefined);
      onClose();
    }
  };

  const handleAiRename = async () => {
    if (!onAiAction) return;
    setIsAiLoading(true);
    try {
      const aiTitle = await onAiAction();
      if (aiTitle) setTitle(aiTitle);
    } finally {
      setIsAiLoading(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('common.rename', { defaultValue: 'Rename' })}
      size="sm"
      autoFocus={false}
      footer={
        <>
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleSave}
            disabled={!title.trim()}
          >
            {t('common.save', { defaultValue: 'Save' })}
          </button>
        </>
      }
    >
      <FormField>
        <div style={{ display: 'flex', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
          <IconPicker
            icon={icon}
            onChange={setIcon}
            defaultIcon={defaultType === 'batch' ? <FileTextIcon /> : <MicIcon />}
          />

          <div style={{ position: 'relative', flex: 1, display: 'flex' }}>
            <input
              ref={inputRef}
              type="text"
              className="input"
              style={{
                flex: 1,
                height: '40px',
                paddingRight: onAiAction ? '40px' : 'var(--spacing-sm)',
              }}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSave()}
              placeholder={t('common.rename_prompt')}
            />
            {onAiAction && (
              <button
                type="button"
                className="btn btn-icon btn-sm"
                onClick={handleAiRename}
                disabled={isAiLoading}
                data-tooltip={t('common.ai_rename', { defaultValue: 'AI Auto-rename' })}
                data-tooltip-pos="top"
                aria-label={t('common.ai_rename', { defaultValue: 'AI Auto-rename' })}
                style={{
                  position: 'absolute',
                  right: '4px',
                  top: 0,
                  bottom: 0,
                  marginTop: 'auto',
                  marginBottom: 'auto',
                  width: '28px',
                  height: '28px',
                  padding: 0,
                  color: 'var(--color-info)',
                  background: 'transparent',
                  border: 'none',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                {isAiLoading ? (
                  <Loader2 className="animate-spin" width={16} height={16} />
                ) : (
                  <SparklesIcon width={16} height={16} />
                )}
              </button>
            )}
          </div>
        </div>
      </FormField>
    </Modal>
  );
}
