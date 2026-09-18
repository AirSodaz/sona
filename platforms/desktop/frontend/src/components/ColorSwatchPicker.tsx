import { Pipette } from 'lucide-react';
import type React from 'react';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PROJECT_COLOR_PRESETS } from '../constants/projects';
import { ColorPicker } from './ColorPicker';

export function getContrastTextColor(hex?: string): string {
  if (!hex) return '#ffffff';
  const cleaned = hex.replace('#', '');
  if (cleaned.length !== 6) return '#ffffff';
  const r = parseInt(cleaned.slice(0, 2), 16);
  const g = parseInt(cleaned.slice(2, 4), 16);
  const b = parseInt(cleaned.slice(4, 6), 16);
  const yiq = (r * 299 + g * 587 + b * 114) / 1000;
  return yiq >= 150 ? '#0f172a' : '#ffffff';
}

export interface ColorSwatchPickerProps {
  value?: string;
  color?: string;
  onChange: (color: string) => void;
  presets?: string[];
  className?: string;
  style?: React.CSSProperties;
  'aria-label'?: string;
  onPickingChange?: (isPicking: boolean) => void;
}

export function ColorSwatchPicker({
  value,
  color,
  onChange,
  presets = PROJECT_COLOR_PRESETS,
  className = '',
  style,
  'aria-label': ariaLabel,
  onPickingChange,
}: ColorSwatchPickerProps): React.JSX.Element {
  const { t } = useTranslation();
  const customColorBtnRef = useRef<HTMLButtonElement>(null);
  const [isColorPickerOpen, setIsColorPickerOpen] = useState(false);
  const isPickingColorRef = useRef(false);

  const currentColor = value ?? color ?? '#ffffff';
  const isCustom = Boolean(
    currentColor && !presets.some((preset) => preset.toLowerCase() === currentColor.toLowerCase())
  );
  const customColorLabel = t('common.custom_color', {
    defaultValue: 'Custom color',
  });
  const iconColor = isCustom ? getContrastTextColor(currentColor) : '#ffffff';

  return (
    <div
      className={`project-color-swatches ${className}`.trim()}
      style={{ marginTop: 0, ...style }}
      aria-label={ariaLabel}
    >
      {presets.map((presetColor) => (
        <button
          key={presetColor}
          type="button"
          className={`project-color-swatch ${currentColor.toLowerCase() === presetColor.toLowerCase() ? 'active' : ''}`}
          style={{ backgroundColor: presetColor }}
          onClick={(e) => {
            e.preventDefault();
            onChange(presetColor);
          }}
          data-tooltip={presetColor}
          data-tooltip-pos="bottom"
          aria-label={presetColor}
        />
      ))}
      <span className="project-color-divider" aria-hidden="true" />
      <button
        type="button"
        ref={customColorBtnRef}
        className={`project-custom-color-swatch ${isCustom ? 'active' : ''}`}
        aria-label={customColorLabel}
        data-tooltip={
          isCustom ? `${customColorLabel}: ${currentColor.toUpperCase()}` : customColorLabel
        }
        data-tooltip-pos="bottom"
        onClick={(e) => {
          e.preventDefault();
          setIsColorPickerOpen((prev) => !prev);
        }}
        style={{
          background: isCustom
            ? currentColor
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
      <ColorPicker
        isOpen={isColorPickerOpen}
        color={currentColor || '#6366F1'}
        onChange={(newColor) => onChange(newColor)}
        onClose={() => setIsColorPickerOpen(false)}
        onPickingChange={(isPicking) => {
          isPickingColorRef.current = isPicking;
          onPickingChange?.(isPicking);
        }}
        anchorRef={customColorBtnRef}
      />
    </div>
  );
}
