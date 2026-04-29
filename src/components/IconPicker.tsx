import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, MicIcon, FileTextIcon, FolderIcon, CodeIcon } from './Icons';

export const SYSTEM_ICONS = [
    { id: 'system:mic', icon: <MicIcon /> },
    { id: 'system:file', icon: <FileTextIcon /> },
    { id: 'system:folder', icon: <FolderIcon /> },
    { id: 'system:code', icon: <CodeIcon /> },
];

export const RECOMMENDED_EMOJIS = ['📄', '🎙️', '📁', '📝', '🗣️', '💡', '⭐️', '🎯', '📌', '📅'];

export function renderIcon(icon?: string, defaultIcon?: React.ReactNode) {
    if (!icon) {
        return defaultIcon || <MicIcon />;
    }
    if (icon.startsWith('system:')) {
        const sysIcon = SYSTEM_ICONS.find(si => si.id === icon);
        return sysIcon ? sysIcon.icon : (defaultIcon || <MicIcon />);
    }
    return <span style={{ fontSize: '1.25rem', lineHeight: 1 }}>{icon}</span>;
}

export interface IconPickerProps {
    icon: string;
    onChange: (icon: string) => void;
    defaultIcon?: React.ReactNode;
}

export function IconPicker({ icon, onChange, defaultIcon }: IconPickerProps): React.JSX.Element {
    const { t } = useTranslation();
    const [isPickerOpen, setIsPickerOpen] = useState(false);
    const [customEmoji, setCustomEmoji] = useState('');
    const pickerRef = useRef<HTMLDivElement>(null);

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

    return (
        <div style={{ position: 'relative' }}>
            <button
                type="button"
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
                onClick={(e) => {
                    e.preventDefault();
                    setIsPickerOpen(!isPickerOpen);
                }}
            >
                {renderIcon(icon, defaultIcon)}
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
                            {t('history.system_icons', { defaultValue: 'System Icons' })}
                        </div>
                        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 'var(--spacing-xs)' }}>
                            {SYSTEM_ICONS.map(si => (
                                <button
                                    key={si.id}
                                    type="button"
                                    className={`btn btn-icon ${icon === si.id ? 'active' : ''}`}
                                    style={{ background: icon === si.id ? 'var(--color-bg-active)' : 'transparent' }}
                                    onClick={(e) => { e.preventDefault(); onChange(si.id); setIsPickerOpen(false); }}
                                    title={si.id.replace('system:', '')}
                                    aria-label={si.id.replace('system:', '')}
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
                            {t('history.emojis', { defaultValue: 'Emojis' })}
                        </div>
                        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 'var(--spacing-xs)' }}>
                            {RECOMMENDED_EMOJIS.map(e => (
                                <button
                                    key={e}
                                    type="button"
                                    className="btn btn-icon"
                                    style={{ fontSize: '1.25rem', background: icon === e ? 'var(--color-bg-active)' : 'transparent' }}
                                    onClick={(event) => { event.preventDefault(); onChange(e); setIsPickerOpen(false); }}
                                    aria-label={e}
                                >
                                    {e}
                                </button>
                            ))}
                        </div>
                    </div>

                    <div style={{ borderTop: '1px solid var(--color-border)', paddingTop: 'var(--spacing-sm)' }}>
                        <input
                            type="text"
                            placeholder={t('history.custom_emoji_placeholder', { defaultValue: 'Custom emoji' })}
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
                                    onChange(val);
                                    setIsPickerOpen(false);
                                    setCustomEmoji('');
                                }
                            }}
                        />
                    </div>
                    <button 
                        type="button"
                        className="btn btn-text btn-sm" 
                        style={{ width: '100%', justifyContent: 'center', fontSize: '0.75rem' }}
                        onClick={(e) => { e.preventDefault(); onChange(''); setIsPickerOpen(false); }}
                    >
                        {t('common.reset', { defaultValue: 'Reset' })}
                    </button>
                </div>
            )}
        </div>
    );
}
