import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { XIcon, SparklesIcon, MicIcon, FileTextIcon } from './Icons';
import { IconPicker } from './IconPicker';

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
    onAiAction
}: RenameModalProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const [title, setTitle] = useState(initialTitle);
    const [icon, setIcon] = useState(initialIcon || '');
    const [isAiLoading, setIsAiLoading] = useState(false);

    const modalRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (!isOpen) {
            return;
        }

        queueMicrotask(() => {
            setTitle(initialTitle);
            setIcon(initialIcon || '');
        });

        const focusTimer = window.setTimeout(() => inputRef.current?.focus(), 50);
        return () => window.clearTimeout(focusTimer);
    }, [isOpen, initialTitle, initialIcon]);

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
        <div className="settings-overlay" onClick={onClose} style={{ zIndex: 2000 }}>
            <div
                ref={modalRef}
                className="dialog-modal"
                onClick={(e) => e.stopPropagation()}
                role="dialog"
                aria-modal="true"
                style={{
                    background: 'var(--color-bg-elevated)',
                    borderRadius: 'var(--radius-lg)',
                    boxShadow: 'var(--shadow-xl)',
                    width: '400px',
                    maxWidth: '90vw',
                    padding: 'var(--spacing-lg)',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 'var(--spacing-md)',
                    border: '1px solid var(--color-border)',
                }}
            >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <h3 style={{ fontSize: '1.125rem', fontWeight: 600, margin: 0 }}>
                        {t('common.rename', { defaultValue: 'Rename' })}
                    </h3>
                    <button
                        type="button"
                        className="btn btn-icon"
                        onClick={onClose}
                        aria-label={t('common.close', { defaultValue: 'Close' })}
                    >
                        <XIcon />
                    </button>
                </div>

                <div style={{ display: 'flex', gap: 'var(--spacing-sm)', alignItems: 'center', marginTop: 'var(--spacing-sm)' }}>
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
                                paddingRight: onAiAction ? '40px' : 'var(--spacing-sm)' 
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
                                title={t('common.ai_rename')}
                                aria-label={t('common.ai_rename')}
                                style={{
                                    position: 'absolute',
                                    right: '4px',
                                    top: '50%',
                                    transform: 'translateY(-50%)',
                                    color: 'var(--color-info)',
                                    background: 'transparent',
                                    border: 'none',
                                    display: 'flex',
                                    alignItems: 'center',
                                    justifyContent: 'center'
                                }}
                            >
                                {isAiLoading ? (
                                    <Loader2 className="spin" width={16} height={16} />
                                ) : (
                                    <SparklesIcon width={16} height={16} />
                                )}
                            </button>
                        )}
                    </div>
                </div>

                <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 'var(--spacing-sm)', marginTop: 'var(--spacing-md)' }}>
                    <button type="button" className="btn btn-secondary" onClick={onClose}>
                        {t('common.cancel')}
                    </button>
                    <button type="button" className="btn btn-primary" onClick={handleSave} disabled={!title.trim()}>
                        {t('common.save', { defaultValue: 'Save' })}
                    </button>
                </div>
            </div>
        </div>
    );
}
