/* eslint-disable react-refresh/only-export-components */

import { Tv } from 'lucide-react';
import type React from 'react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ColorSwatchPicker } from './ColorSwatchPicker';
import { ChevronDownIcon, CodeIcon, FileTextIcon, FolderIcon, MicIcon } from './Icons';
import { ModalPortal } from './ModalPortal';
import { Button, IconButton } from './ui/Button';

export const MEDIA_SERVER_ICON_EMBY = 'system:emby';
export const MEDIA_SERVER_ICON_JELLYFIN = 'system:jellyfin';

export function getMediaServerIconId(serverType?: 'emby' | 'jellyfin'): string {
  return serverType === 'jellyfin' ? MEDIA_SERVER_ICON_JELLYFIN : MEDIA_SERVER_ICON_EMBY;
}

export function getMediaServerIconMeta(icon?: string | null): {
  serverType: 'emby' | 'jellyfin';
  label: 'Emby' | 'Jellyfin';
  color: string;
} | null {
  if (icon === MEDIA_SERVER_ICON_JELLYFIN) {
    return {
      serverType: 'jellyfin',
      label: 'Jellyfin',
      color: '#a855f7',
    };
  }
  if (icon === MEDIA_SERVER_ICON_EMBY) {
    return {
      serverType: 'emby',
      label: 'Emby',
      color: 'var(--color-success, #52c41a)',
    };
  }
  return null;
}

export const SYSTEM_ICONS = [
  {
    id: 'system:mic',
    nameKey: 'history.icon_mic',
    defaultName: 'Microphone',
    get icon() {
      return <MicIcon />;
    },
  },
  {
    id: 'system:file',
    nameKey: 'history.icon_file',
    defaultName: 'Document',
    get icon() {
      return <FileTextIcon />;
    },
  },
  {
    id: 'system:folder',
    nameKey: 'history.icon_folder',
    defaultName: 'Folder',
    get icon() {
      return <FolderIcon />;
    },
  },
  {
    id: 'system:code',
    nameKey: 'history.icon_code',
    defaultName: 'Code',
    get icon() {
      return <CodeIcon />;
    },
  },
];

export const RECOMMENDED_EMOJIS = ['📄', '🎙️', '📁', '📝', '🗣️', '💡', '⭐️', '🎯', '📌', '📅'];

export function renderSystemIcon(icon: string, color?: string): React.ReactNode | null {
  const mediaServerMeta = getMediaServerIconMeta(icon);
  if (mediaServerMeta) {
    const resolvedColor = color || mediaServerMeta.color;
    return (
      <span
        style={{
          color: resolvedColor,
          display: 'inline-flex',
          alignItems: 'center',
        }}
      >
        <Tv size={16} />
      </span>
    );
  }

  const sysIcon = SYSTEM_ICONS.find((si) => si.id === icon);
  if (!sysIcon) {
    return null;
  }
  if (color) {
    return (
      <span style={{ color, display: 'inline-flex', alignItems: 'center' }}>{sysIcon.icon}</span>
    );
  }
  return sysIcon.icon;
}

export function renderIcon(icon?: string, defaultIcon?: React.ReactNode, color?: string) {
  if (!icon) {
    const fallback = defaultIcon || <MicIcon />;
    if (color) {
      return (
        <span style={{ color, display: 'inline-flex', alignItems: 'center' }}>{fallback}</span>
      );
    }
    return fallback;
  }
  if (icon.startsWith('system:')) {
    const resolved = renderSystemIcon(icon, color);
    if (resolved) {
      return resolved;
    }
    const fallback = defaultIcon || <MicIcon />;
    if (color) {
      return (
        <span style={{ color, display: 'inline-flex', alignItems: 'center' }}>{fallback}</span>
      );
    }
    return fallback;
  }
  return <span style={{ fontSize: '1.25rem', lineHeight: 1 }}>{icon}</span>;
}

export interface IconPickerProps {
  icon: string;
  onChange: (icon: string) => void;
  defaultIcon?: React.ReactNode;
  resetIconValue?: string;
  color?: string;
  onColorChange?: (color: string) => void;
}

export function IconPicker({
  icon,
  onChange,
  defaultIcon,
  resetIconValue = '',
  color,
  onColorChange,
}: IconPickerProps): React.JSX.Element {
  const { t } = useTranslation();
  const [isPickerOpen, setIsPickerOpen] = useState(false);
  const [customEmoji, setCustomEmoji] = useState('');
  const buttonRef = useRef<HTMLButtonElement>(null);
  const pickerRef = useRef<HTMLDivElement>(null);
  const [popoverStyle, setPopoverStyle] = useState<React.CSSProperties>({});
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
      <Button
        ref={buttonRef}
        tier="secondary"
        aria-label={
          icon && !icon.startsWith('system:')
            ? undefined
            : t('history.choose_icon', { defaultValue: 'Choose icon' })
        }
        tooltip={t('history.choose_icon', { defaultValue: 'Choose icon' })}
        tooltipPos="top"
        style={{
          gap: 'var(--spacing-xs)',
          minWidth: '60px',
          height: '40px',
          borderColor: color ? `color-mix(in srgb, ${color} 35%, var(--color-border))` : undefined,
          background: color
            ? `color-mix(in srgb, ${color} 8%, var(--color-bg-secondary))`
            : undefined,
        }}
        onClick={(e) => {
          e.preventDefault();
          setIsPickerOpen(!isPickerOpen);
        }}
      >
        {renderIcon(icon, defaultIcon, color)}
        <ChevronDownIcon style={{ width: '12px', height: '12px' }} />
      </Button>

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
                <div
                  style={{
                    fontSize: '0.75rem',
                    fontWeight: 600,
                    color: 'var(--color-text-muted)',
                    marginBottom: 'var(--spacing-xs)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.025em',
                  }}
                >
                  {t('projects.tag_color', { defaultValue: 'Color' })}
                </div>
                <ColorSwatchPicker
                  color={color}
                  onChange={onColorChange}
                  onPickingChange={(isPicking) => {
                    isPickingColorRef.current = isPicking;
                  }}
                />
              </div>
            )}

            <div>
              <div
                style={{
                  fontSize: '0.75rem',
                  fontWeight: 600,
                  color: 'var(--color-text-muted)',
                  marginBottom: 'var(--spacing-xs)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.025em',
                }}
              >
                {t('history.system_icons', { defaultValue: 'System Icons' })}
              </div>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(4, 1fr)',
                  gap: 'var(--spacing-xs)',
                }}
              >
                {SYSTEM_ICONS.map((si) => {
                  const label = t(si.nameKey, { defaultValue: si.defaultName });
                  return (
                    <IconButton
                      key={si.id}
                      tier="ghost"
                      size="md"
                      active={icon === si.id}
                      icon={si.icon}
                      style={{
                        color: color || 'inherit',
                      }}
                      onClick={(event) => {
                        event.preventDefault();
                        onChange(si.id);
                        setIsPickerOpen(false);
                      }}
                      tooltip={label}
                      tooltipPos="top"
                      aria-label={label}
                    />
                  );
                })}
              </div>
            </div>

            <div>
              <div
                style={{
                  fontSize: '0.75rem',
                  fontWeight: 600,
                  color: 'var(--color-text-muted)',
                  marginBottom: 'var(--spacing-xs)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.025em',
                }}
              >
                {t('history.emojis', { defaultValue: 'Emojis' })}
              </div>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(5, 1fr)',
                  gap: 'var(--spacing-xs)',
                }}
              >
                {RECOMMENDED_EMOJIS.map((e) => (
                  <IconButton
                    key={e}
                    tier="ghost"
                    size="md"
                    active={icon === e}
                    icon={e}
                    style={{ fontSize: '1.25rem' }}
                    onClick={(event) => {
                      event.preventDefault();
                      onChange(e);
                      setIsPickerOpen(false);
                    }}
                    aria-label={e}
                  />
                ))}
              </div>
            </div>

            <div
              style={{
                borderTop: '1px solid var(--color-border)',
                paddingTop: 'var(--spacing-sm)',
              }}
            >
              <input
                type="text"
                placeholder={t('history.custom_emoji_placeholder', {
                  defaultValue: 'Custom emoji',
                })}
                aria-label={t('history.custom_emoji_placeholder', { defaultValue: 'Custom emoji' })}
                maxLength={2}
                style={{
                  width: '100%',
                  padding: 'var(--spacing-xs) var(--spacing-sm)',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--color-border)',
                  background: 'var(--color-bg-input)',
                  fontSize: '0.875rem',
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
            <Button
              tier="ghost"
              size="sm"
              fullWidth
              onClick={(e) => {
                e.preventDefault();
                onChange(resetIconValue);
                setIsPickerOpen(false);
              }}
              tooltip={t('history.reset_icon', { defaultValue: 'Reset icon' })}
              tooltipPos="top"
            >
              {t('common.reset', { defaultValue: 'Reset' })}
            </Button>
          </div>
        </ModalPortal>
      )}
    </div>
  );
}
