import { Pipette } from 'lucide-react';
import type React from 'react';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { COLOR_PRESETS, DEFAULT_PROJECT_COLOR } from '../constants/colors';
import { ColorPicker } from './ColorPicker';
import { Tooltip } from './ui/Tooltip';

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
  presets = [...COLOR_PRESETS],
  className = '',
  style,
  'aria-label': ariaLabel,
  onPickingChange,
}: ColorSwatchPickerProps): React.JSX.Element {
  const { t } = useTranslation();
  const customColorBtnRef = useRef<HTMLButtonElement>(null);
  const [isColorPickerOpen, setIsColorPickerOpen] = useState(false);
  const isPickingColorRef = useRef(false);

  const currentColor = value ?? color ?? DEFAULT_PROJECT_COLOR;
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
        <Tooltip key={presetColor} content={presetColor} placement="bottom">
          <button
            type="button"
            className={`project-color-swatch ${currentColor.toLowerCase() === presetColor.toLowerCase() ? 'active' : ''}`}
            style={{ backgroundColor: presetColor }}
            onClick={(e) => {
              e.preventDefault();
              onChange(presetColor);
            }}
            aria-label={presetColor}
          />
        </Tooltip>
      ))}
      <span className="project-color-divider" aria-hidden="true" />
      <Tooltip
        content={isCustom ? `${customColorLabel}: ${currentColor.toUpperCase()}` : customColorLabel}
        placement="bottom"
      >
        <button
          type="button"
          ref={customColorBtnRef}
          className={`project-custom-color-swatch ${isCustom ? 'active' : ''}`}
          aria-label={customColorLabel}
          onClick={(e) => {
            e.preventDefault();
            setIsColorPickerOpen((prev) => !prev);
          }}
          style={{
            background: isCustom
              ? currentColor
              : 'conic-gradient(from 180deg at 50% 50%, #C96F6B 0deg, #E2A03F 72deg, #4EC39D 144deg, #5B82E8 216deg, #8C78B8 288deg, #C96F6B 360deg)',
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
      </Tooltip>
      <ColorPicker
        isOpen={isColorPickerOpen}
        color={currentColor || DEFAULT_PROJECT_COLOR}
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
