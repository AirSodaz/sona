import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2 } from 'lucide-react';
import { XIcon, SparklesIcon, MicIcon, FileTextIcon, FolderIcon, CodeIcon, ChevronDownIcon } from './Icons';

interface RenameModalProps {
    isOpen: boolean;
    onClose: () => void;
    initialTitle: string;
    initialIcon?: string;
    defaultType?: 'recording' | 'batch';
    onRename: (title: string, icon?: string) => void;
    onAiAction?: () => Promise<string>;
}

const SYSTEM_ICONS = [
    { id: 'system:mic', icon: <MicIcon /> },
    { id: 'system:file', icon: <FileTextIcon /> },
    { id: 'system:folder', icon: <FolderIcon /> },
    { id: 'system:code', icon: <CodeIcon /> },
];

const RECOMMENDED_EMOJIS = ['📄', '🎙️', '📁', '📝', '🗣️', '💡', '⭐️', '🎯', '📌', '📅'];

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
    const [isPickerOpen, setIsPickerOpen] = useState(false);
    const [isAiLoading, setIsAiLoading] = useState(false);
    const [customEmoji, setCustomEmoji] = useState('');

    const modalRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const pickerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (isOpen) {
            setTitle(initialTitle);
            setIcon(initialIcon || '');
            setIsPickerOpen(false);
            setTimeout(() => inputRef.current?.focus(), 50);
        }
    }, [isOpen, initialTitle, initialIcon]);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (pickerRef.current && !pickerRef.current.contains(event.target as Node)) {
                setIsPickerOpen(false);
            }
        };

        if (isPickerOpen) {
            document.addEventListener('mousedown', handleClickOutside);
        }
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [isPickerOpen]);

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

    const renderCurrentIcon = () => {
        if (!icon) {
            return defaultType === 'batch' ? <FileTextIcon /> : <MicIcon />;
        }
        if (icon.startsWith('system:')) {
            const sysIcon = SYSTEM_ICONS.find(si => si.id === icon);
            return sysIcon ? sysIcon.icon : <MicIcon />;
        }
        return <span style={{ fontSize: '1.25rem' }}>{icon}</span>;
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
                    <button className="btn btn-icon" onClick={onClose}>
                        <XIcon />
                    </button>
                </div>

                <div style={{ display: 'flex', gap: 'var(--spacing-sm)', alignItems: 'center', marginTop: 'var(--spacing-sm)' }}>
                    <div style={{ position: 'relative' }}>
                        <button
                            className="btn btn-secondary"
                            style={{ 
                                padding: 'var(--spacing-xs) var(--spacing-sm)', 
                                display: 'flex', 
                                alignItems: 'center', 
                                gap: 'var(--spacing-xs)',
                                minWidth: '60px',
                                height: '40px',
                                justifyContent: 'center'
                            }}
                            onClick={() => setIsPickerOpen(!isPickerOpen)}
                        >
                            {renderCurrentIcon()}
                            <ChevronDownIcon style={{ width: '12px', height: '12px' }} />
                        </button>

                        {isPickerOpen && (
                            <div
                                ref={pickerRef}
                                style={{
                                    position: 'absolute',
                                    top: '100%',
                                    left: 0,
                                    marginTop: 'var(--spacing-xs)',
                                    background: 'var(--color-bg-elevated)',
                                    border: '1px solid var(--color-border)',
                                    borderRadius: 'var(--radius-md)',
                                    boxShadow: 'var(--shadow-lg)',
                                    padding: 'var(--spacing-md)',
                                    zIndex: 2100,
                                    width: '260px',
                                    display: 'flex',
                                    flexDirection: 'column',
                                    gap: 'var(--spacing-sm)'
                                }}
                            >
                                <div>
                                    <div style={{ 
                                        fontSize: '0.75rem', 
                                        fontWeight: 600, 
                                        color: 'var(--color-text-muted)', 
                                        marginBottom: 'var(--spacing-xs)',
                                        textTransform: 'uppercase',
                                        letterSpacing: '0.025em'
                                    }}>
                                        {t('history.system_icons')}
                                    </div>
                                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 'var(--spacing-xs)' }}>
                                        {SYSTEM_ICONS.map(si => (
                                            <button
                                                key={si.id}
                                                className={`btn btn-icon ${icon === si.id ? 'active' : ''}`}
                                                style={{ background: icon === si.id ? 'var(--color-bg-active)' : 'transparent' }}
                                                onClick={() => { setIcon(si.id); setIsPickerOpen(false); }}
                                                title={si.id.replace('system:', '')}
                                            >
                                                {si.icon}
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                <div>
                                    <div style={{ 
                                        fontSize: '0.75rem', 
                                        fontWeight: 600, 
                                        color: 'var(--color-text-muted)', 
                                        marginBottom: 'var(--spacing-xs)',
                                        textTransform: 'uppercase',
                                        letterSpacing: '0.025em'
                                    }}>
                                        {t('history.emojis')}
                                    </div>
                                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 'var(--spacing-xs)' }}>
                                        {RECOMMENDED_EMOJIS.map(e => (
                                            <button
                                                key={e}
                                                className="btn btn-icon"
                                                style={{ fontSize: '1.25rem', background: icon === e ? 'var(--color-bg-active)' : 'transparent' }}
                                                onClick={() => { setIcon(e); setIsPickerOpen(false); }}
                                            >
                                                {e}
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                <div style={{ borderTop: '1px solid var(--color-border)', paddingTop: 'var(--spacing-sm)' }}>
                                    <input
                                        type="text"
                                        placeholder={t('history.custom_emoji_placeholder')}
                                        maxLength={2}
                                        style={{ 
                                            width: '100%', 
                                            padding: 'var(--spacing-xs) var(--spacing-sm)', 
                                            borderRadius: 'var(--radius-sm)',
                                            border: '1px solid var(--color-border)',
                                            background: 'var(--color-bg-input)',
                                            fontSize: '0.875rem'
                                        }}
                                        value={customEmoji}
                                        onChange={(e) => {
                                            const val = e.target.value;
                                            setCustomEmoji(val);
                                            if (val && val.length >= 1) {
                                                setIcon(val);
                                                setIsPickerOpen(false);
                                                setCustomEmoji('');
                                            }
                                        }}
                                    />
                                </div>
                                <button 
                                    className="btn btn-text btn-sm" 
                                    style={{ width: '100%', justifyContent: 'center', fontSize: '0.75rem' }}
                                    onClick={() => { setIcon(''); setIsPickerOpen(false); }}
                                >
                                    {t('common.reset', { defaultValue: 'Reset' })}
                                </button>
                            </div>
                        )}
                    </div>

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
                                className="btn btn-icon btn-sm"
                                onClick={handleAiRename}
                                disabled={isAiLoading}
                                title={t('common.ai_rename')}
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
                    <button className="btn btn-secondary" onClick={onClose}>
                        {t('common.cancel')}
                    </button>
                    <button className="btn btn-primary" onClick={handleSave} disabled={!title.trim()}>
                        {t('common.save', { defaultValue: 'Save' })}
                    </button>
                </div>
            </div>
        </div>
    );
}
