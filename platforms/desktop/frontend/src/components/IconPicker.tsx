/* eslint-disable react-refresh/only-export-components */

import React, { useState, useEffect, useLayoutEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Pipette } from 'lucide-react';
import { ChevronDownIcon, MicIcon, FileTextIcon, FolderIcon, CodeIcon } from './Icons';
import { PROJECT_COLOR_PRESETS } from '../constants/projects';
import { ModalPortal } from './ModalPortal';
import { ColorPicker } from './ColorPicker';

function getContrastTextColor(hex?: string): string {
    if (!hex) return '#ffffff';
    const cleaned = hex.replace('#', '');
    if (cleaned.length !== 6) return '#ffffff';
    const r = parseInt(cleaned.slice(0, 2), 16);
    const g = parseInt(cleaned.slice(2, 4), 16);
    const b = parseInt(cleaned.slice(4, 6), 16);
    const yiq = (r * 299 + g * 587 + b * 114) / 1000;
    return yiq >= 150 ? '#0f172a' : '#ffffff';
}

export const SYSTEM_ICONS = [
    { id: 'system:mic', nameKey: 'history.icon_mic', defaultName: 'Microphone', get icon() { return <MicIcon />; } },
    { id: 'system:file', nameKey: 'history.icon_file', defaultName: 'Document', get icon() { return <FileTextIcon />; } },
    { id: 'system:folder', nameKey: 'history.icon_folder', defaultName: 'Folder', get icon() { return <FolderIcon />; } },
    { id: 'system:code', nameKey: 'history.icon_code', defaultName: 'Code', get icon() { return <CodeIcon />; } },
];

export const RECOMMENDED_EMOJIS = ['📄', '🎙️', '📁', '📝', '🗣️', '💡', '⭐️', '🎯', '📌', '📅'];

export function renderIcon(icon?: string, defaultIcon?: React.ReactNode, color?: string) {
    if (!icon) {
        const fallback = defaultIcon || <MicIcon />;
        if (color) {
            return <span style={{ color, display: 'inline-flex', alignItems: 'center' }}>{fallback}</span>;
        }
        return fallback;
    }
    if (icon.startsWith('system:')) {
        const sysIcon = SYSTEM_ICONS.find(si => si.id === icon);
        const resolved = sysIcon ? sysIcon.icon : (defaultIcon || <MicIcon />);
        if (color) {
            return <span style={{ color, display: 'inline-flex', alignItems: 'center' }}>{resolved}</span>;
        }
        return resolved;
    }
    return <span style={{ fontSize: '1.25rem', lineHeight: 1 }}>{icon}</span>;
}

export interface IconPickerProps {
    icon: string;
    onChange: (icon: string) => void;
    defaultIcon?: React.ReactNode;
    color?: string;
    onColorChange?: (color: string) => void;
}

export function IconPicker({ icon, onChange, defaultIcon, color, onColorChange }: IconPickerProps): React.JSX.Element {
    const { t } = useTranslation();
    const [isPickerOpen, setIsPickerOpen] = useState(false);
    const [customEmoji, setCustomEmoji] = useState('');
    const buttonRef = useRef<HTMLButtonElement>(null);
    const pickerRef = useRef<HTMLDivElement>(null);
    const customColorBtnRef = useRef<HTMLButtonElement>(null);
    const [popoverStyle, setPopoverStyle] = useState<React.CSSProperties>({});
    const [isColorPickerOpen, setIsColorPickerOpen] = useState(false);
    const isPickingColorRef = useRef(false);
    useLayoutEffect(() => {
        if (isPickerOpen && buttonRef.current) {
            const rect = buttonRef.current.getBoundingClientRect();
            const popoverHeight = color ? 340 : 260;
            const spaceBelow = window.innerHeight - rect.bottom;
            const spaceAbove = rect.top;

            let newPosition: 'bottom' | 'top' = 'bottom';
            if (spaceBelow < popoverHeight && spaceAbove > spaceBelow) {
                newPosition = 'top';
            }

            const popoverWidth = 268;
            const maxLeft = Math.max(8, window.innerWidth - popoverWidth - 12);
            const left = Math.max(8, Math.min(rect.left, maxLeft));

            const style: React.CSSProperties = {
                position: 'fixed',
                left,
                width: `${popoverWidth}px`,
                zIndex: 2500,
            };

            if (newPosition === 'top') {
                style.bottom = Math.max(8, window.innerHeight - rect.top + 6);
            } else {
                style.top = Math.max(8, rect.bottom + 6);
            }

            setPopoverStyle(style);
        }
    }, [isPickerOpen, color]);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (isPickingColorRef.current) {
                return;
            }
            const target = event.target as Node;
            if ((target as Element)?.closest?.('.sona-color-picker-popover')) {
                return;
            }
            if (
                pickerRef.current &&
                !pickerRef.current.contains(target) &&
                buttonRef.current &&
                !buttonRef.current.contains(target)
            ) {
                setIsPickerOpen(false);
                setIsColorPickerOpen(false);
            }
        };

        const handleScroll = (event: Event) => {
            if (isPickingColorRef.current) {
                return;
            }
            const target = event.target as Node;
            if ((target as Element)?.closest?.('.sona-color-picker-popover')) {
                return;
            }
            if (pickerRef.current && !pickerRef.current.contains(event.target as Node)) {
                setIsPickerOpen(false);
                setIsColorPickerOpen(false);
            }
        };

        if (isPickerOpen) {
            document.addEventListener('mousedown', handleClickOutside);
            window.addEventListener('scroll', handleScroll, true);
        }
        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
            window.removeEventListener('scroll', handleScroll, true);
        };
    }, [isPickerOpen]);
    return (
        <div style={{ position: 'relative' }}>
            <button
                ref={buttonRef}
                type="button"
                className="btn btn-secondary"
                data-tooltip={t('history.choose_icon', { defaultValue: 'Choose icon' })}
                data-tooltip-pos="top"
                style={{
                    display: 'flex', 
                    alignItems: 'center', 
                    gap: 'var(--spacing-xs)',
                    minWidth: '60px',
                    height: '40px',
                    justifyContent: 'center',
                    borderColor: color ? `color-mix(in srgb, ${color} 35%, var(--color-border))` : undefined,
                    background: color ? `color-mix(in srgb, ${color} 8%, var(--color-bg-secondary))` : undefined,
                }}
                onClick={(e) => {
                    e.preventDefault();
                    setIsPickerOpen(!isPickerOpen);
                }}
            >
                {renderIcon(icon, defaultIcon, color)}
                <ChevronDownIcon style={{ width: '12px', height: '12px' }} />
            </button>

            {isPickerOpen && (
                <ModalPortal>
                    <div
                        ref={pickerRef}
                        className="icon-picker-popover"
                        style={{
                            ...popoverStyle,
                            background: 'var(--color-bg-elevated)',
                            border: '1px solid var(--color-border)',
                            borderRadius: 'var(--radius-md)',
                            boxShadow: 'var(--shadow-xl)',
                            padding: 'var(--spacing-md)',
                            display: 'flex',
                            flexDirection: 'column',
                            gap: 'var(--spacing-sm)',
                            maxHeight: 'calc(100vh - 32px)',
                            overflowY: 'auto',
                        }}
                    >
                    {color && onColorChange && (
                        <div>
                            <div style={{
                                fontSize: '0.75rem',
                                fontWeight: 600,
                                color: 'var(--color-text-muted)',
                                marginBottom: 'var(--spacing-xs)',
                                textTransform: 'uppercase',
                                letterSpacing: '0.025em',
                            }}>
                                {t('projects.tag_color', { defaultValue: 'Color' })}
                            </div>
                            <div className="project-color-swatches" style={{ marginTop: 0 }}>
                                {PROJECT_COLOR_PRESETS.map((presetColor) => (
                                    <button
                                        key={presetColor}
                                        type="button"
                                        className={`project-color-swatch ${color.toLowerCase() === presetColor.toLowerCase() ? 'active' : ''}`}
                                        style={{ backgroundColor: presetColor }}
                                        onClick={(e) => {
                                            e.preventDefault();
                                            onColorChange(presetColor);
                                        }}
                                        data-tooltip={presetColor}
                                        data-tooltip-pos="bottom"
                                        aria-label={presetColor}
                                    />
                                ))}
                                <span className="project-color-divider" aria-hidden="true" />
                                {(() => {
                                    const isCustom = Boolean(
                                        color && !PROJECT_COLOR_PRESETS.some((preset) => preset.toLowerCase() === color.toLowerCase())
                                    );
                                    const customColorLabel = t('common.custom_color', { defaultValue: 'Custom color' });
                                    const iconColor = isCustom ? getContrastTextColor(color) : '#ffffff';
                                    return (
                                        <button
                                            type="button"
                                            ref={customColorBtnRef}
                                            className={`project-custom-color-swatch ${isCustom ? 'active' : ''}`}
                                            aria-label={customColorLabel}
                                            data-tooltip={isCustom ? `${customColorLabel}: ${color.toUpperCase()}` : customColorLabel}
                                            data-tooltip-pos="bottom"
                                            onClick={(e) => {
                                                e.preventDefault();
                                                setIsColorPickerOpen(prev => !prev);
                                            }}
                                            style={{
                                                background: isCustom
                                                    ? color
                                                    : 'conic-gradient(from 180deg at 50% 50%, #f43f5e 0deg, #ec4899 45deg, #8b5cf6 90deg, #6366f1 135deg, #06b6d4 180deg, #10b981 225deg, #f59e0b 270deg, #f43f5e 360deg)',
                                            }}
                                        >
                                            <span className="project-custom-color-badge" aria-hidden="true">
                                                <Pipette
                                                    size={11}
                                                    strokeWidth={2.4}
                                                    style={{
                                                        color: iconColor,
                                                        filter: isCustom ? undefined : 'drop-shadow(0 1px 1px rgba(0,0,0,0.5))',
                                                    }}
                                                />
                                            </span>
                                        </button>
                                    );
                                })()}
                            </div>
                            <ColorPicker
                                isOpen={isColorPickerOpen}
                                color={color || '#6366F1'}
                                onChange={(newColor) => onColorChange(newColor)}
                                onClose={() => setIsColorPickerOpen(false)}
                                onPickingChange={(isPicking) => {
                                    isPickingColorRef.current = isPicking;
                                }}
                                anchorRef={customColorBtnRef}
                            />
                        </div>
                    )}

                    <div>
                        <div style={{
                            fontSize: '0.75rem',
                            fontWeight: 600,
                            color: 'var(--color-text-muted)',
                            marginBottom: 'var(--spacing-xs)',
                            textTransform: 'uppercase',
                            letterSpacing: '0.025em',
                        }}>
                            {t('history.system_icons', { defaultValue: 'System Icons' })}
                        </div>
                        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 'var(--spacing-xs)' }}>
                            {SYSTEM_ICONS.map(si => {
                                const label = t(si.nameKey, { defaultValue: si.defaultName });
                                return (
                                    <button
                                        key={si.id}
                                        type="button"
                                        className={`btn btn-icon ${icon === si.id ? 'active' : ''}`}
                                        style={{
                                            background: icon === si.id ? 'var(--color-bg-active)' : 'transparent',
                                            color: color || 'inherit',
                                        }}
                                        onClick={(event) => {
                                            event.preventDefault();
                                            onChange(si.id);
                                            setIsPickerOpen(false);
                                        }}
                                        data-tooltip={label}
                                        data-tooltip-pos="top"
                                        aria-label={label}
                                    >
                                        {si.icon}
                                    </button>
                                );
                            })}
                        </div>
                    </div>

                    <div>
                        <div style={{
                            fontSize: '0.75rem',
                            fontWeight: 600,
                            color: 'var(--color-text-muted)',
                            marginBottom: 'var(--spacing-xs)',
                            textTransform: 'uppercase',
                            letterSpacing: '0.025em',
                        }}>
                            {t('history.emojis', { defaultValue: 'Emojis' })}
                        </div>
                        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 'var(--spacing-xs)' }}>
                            {RECOMMENDED_EMOJIS.map(e => (
                                <button
                                    key={e}
                                    type="button"
                                    className="btn btn-icon"
                                    style={{
                                        fontSize: '1.25rem',
                                        background: icon === e ? 'var(--color-bg-active)' : 'transparent',
                                    }}
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
                            aria-label={t('history.custom_emoji_placeholder', { defaultValue: 'Custom emoji' })}
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
                        data-tooltip={t('history.reset_icon', { defaultValue: 'Reset icon' })}
                        data-tooltip-pos="top"
                    >
                        {t('common.reset', { defaultValue: 'Reset' })}
                    </button>
                    </div>
                </ModalPortal>
            )}
        </div>
    );
}
