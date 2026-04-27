import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FilePenLine } from 'lucide-react';

interface SettingsShortcutInputProps {
    value: string;
    onChange: (newValue: string) => void;
}

export function SettingsShortcutInput({
    value,
    onChange,
}: SettingsShortcutInputProps): React.JSX.Element {
    const [isRecording, setIsRecording] = useState(false);
    const [isEditing, setIsEditing] = useState(false);
    const [tempValue, setTempValue] = useState(value);
    const { t } = useTranslation();

    useEffect(() => {
        if (!isEditing) {
            setTempValue(value);
        }
    }, [value, isEditing]);

    const finalizeShortcut = (e: React.KeyboardEvent) => {
        const keys: string[] = [];

        if (e.ctrlKey || e.key === 'Control') keys.push('Ctrl');
        if (e.altKey || e.key === 'Alt') keys.push('Alt');
        if (e.shiftKey || e.key === 'Shift') keys.push('Shift');
        if (e.metaKey || e.key === 'Meta') keys.push('Meta');

        const isModifier = ['Control', 'Alt', 'Shift', 'Meta'].includes(e.key);

        let mainKey = '';
        if (!isModifier) {
            if (e.code === 'Space' || e.key === ' ') {
                mainKey = 'Space';
            } else if (e.code && e.code.startsWith('Key')) {
                mainKey = e.code.replace('Key', '');
            } else if (e.code && e.code.startsWith('Digit')) {
                mainKey = e.code.replace('Digit', '');
            } else if (e.key && e.key.length === 1) {
                mainKey = e.key.toUpperCase();
            } else if (
                e.key &&
                e.key !== 'Unidentified' &&
                e.key !== 'Process' &&
                e.key !== 'Dead'
            ) {
                mainKey = e.key;
            }
        }

        if (mainKey) {
            keys.push(mainKey);
        }

        const uniqueKeys = Array.from(new Set(keys));
        if (uniqueKeys.length > 0) {
            onChange(uniqueKeys.join(' + '));
            setIsRecording(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (!isRecording) return;
        e.preventDefault();
        e.stopPropagation();

        if (e.key === 'Escape') {
            setIsRecording(false);
            return;
        }

        const isModifier = ['Control', 'Alt', 'Shift', 'Meta'].includes(e.key);
        if (!isModifier) {
            finalizeShortcut(e);
        }
    };

    const handleKeyUp = (e: React.KeyboardEvent) => {
        if (!isRecording) return;
        e.preventDefault();
        e.stopPropagation();

        finalizeShortcut(e);
    };

    if (isEditing) {
        return (
            <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                <input
                    type="text"
                    value={tempValue}
                    onChange={(e) => setTempValue(e.target.value)}
                    onBlur={() => {
                        onChange(tempValue);
                        setIsEditing(false);
                    }}
                    onKeyDown={(e) => {
                        switch (e.key) {
                            case 'Enter':
                                onChange(tempValue);
                                setIsEditing(false);
                                break;
                            case 'Escape':
                                setTempValue(value);
                                setIsEditing(false);
                                break;
                        }
                    }}
                    autoFocus
                    style={{
                        width: '120px',
                        padding: '4px 8px',
                        textAlign: 'center',
                        background: 'var(--color-bg-secondary)',
                        color: 'var(--color-text-primary)',
                        border: '1px solid var(--color-border)',
                        borderRadius: '6px',
                    }}
                />
            </div>
        );
    }

    const displayValue = value.trim() || t('shortcuts.not_set', { defaultValue: 'Not set' });

    return (
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <button
                type="button"
                className={`btn ${isRecording ? 'btn-primary' : 'btn-secondary'}`}
                style={{ minWidth: '120px', display: 'flex', justifyContent: 'center' }}
                onClick={() => setIsRecording(true)}
                onKeyDown={handleKeyDown}
                onKeyUp={handleKeyUp}
                onBlur={() => setIsRecording(false)}
            >
                {isRecording ? t('common.recording', { defaultValue: 'Recording...' }) : displayValue}
            </button>
            <button
                type="button"
                onClick={() => setIsEditing(true)}
                data-tooltip={t('shortcuts.edit_manually', { defaultValue: 'Edit manually' })}
                data-tooltip-pos="top"
                style={{
                    padding: '4px',
                    background: 'transparent',
                    border: 'none',
                    color: 'var(--color-text-muted)',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                }}
            >
                <FilePenLine size={16} />
            </button>
        </div>
    );
}
