import type React from 'react';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTabKeyboardNavigation } from './useTabKeyboardNavigation';
import './SettingsShared.css';

export type SecondaryTabVariant = 'card' | 'segmented';

export interface SecondaryTabItem<T extends string = string> {
  value: T;
  label: React.ReactNode;
  description?: React.ReactNode;
  icon?: React.ReactNode;
  tooltip?: string;
  tooltipPos?: 'top' | 'bottom' | 'left' | 'right';
  id?: string;
  panelId?: string;
  ariaLabel?: string;
  disabled?: boolean;
  className?: string;
}

export interface SecondaryTabsProps<T extends string = string> {
  variant?: SecondaryTabVariant;
  items: SecondaryTabItem<T>[];
  activeTab: T;
  onChange: (value: T) => void;
  ariaLabel?: string;
  id?: string;
  columns?: 2 | 3 | 'two-columns' | 'three-columns';
  idPrefix?: string;
  getPanelId?: (value: T) => string;
  className?: string;
  style?: React.CSSProperties;
  bordered?: boolean;
  keyboardNavigation?: boolean;
  animated?: boolean;
  size?: 'md' | 'sm';
  role?: 'tablist' | 'radiogroup';
  disabled?: boolean;
  tooltip?: string;
  tooltipPos?: 'top' | 'bottom' | 'left' | 'right';
  tooltipMultiline?: boolean;
}

/**
 * Unified secondary tabs component for Settings and sub-panels.
 * Supports both card-style tabs (ScenarioCardTabs) and segmented pills (SegmentedSubTabs).
 */
export function SecondaryTabs<T extends string = string>({
  variant = 'card',
  items,
  activeTab,
  onChange,
  ariaLabel,
  id,
  columns,
  idPrefix,
  getPanelId,
  className = '',
  style,
  bordered = false,
  keyboardNavigation = true,
  animated = true,
  size = 'md',
  role = 'tablist',
  disabled = false,
  tooltip,
  tooltipPos = 'top',
  tooltipMultiline = false,
}: SecondaryTabsProps<T>): React.JSX.Element {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const { registerButtonRef, handleKeyDown } = useTabKeyboardNavigation({
    items,
    activeTab,
    onChange,
    enabled: keyboardNavigation && !disabled,
  });

  const [sliderStyle, setSliderStyle] = useState<React.CSSProperties>({
    opacity: 0,
    pointerEvents: 'none',
  });
  const [hasMeasured, setHasMeasured] = useState(false);
  const lastContainerSize = useRef<{ width: number; height: number } | null>(null);

  const updatePosition = useCallback(
    (targetTab?: T) => {
      if (!animated) return;
      const container = containerRef.current;
      const activeButton =
        (targetTab
          ? container?.querySelector<HTMLButtonElement>(`button[data-tab-value="${targetTab}"]`)
          : null) ??
        container?.querySelector<HTMLButtonElement>('button[role="tab"][aria-selected="true"]') ??
        null;
      if (!container || !activeButton) {
        setHasMeasured(false);
        setSliderStyle({ opacity: 0, pointerEvents: 'none' });
        return;
      }

      const containerRect = container.getBoundingClientRect();
      const activeRect = activeButton.getBoundingClientRect();

      if (activeRect.width === 0 || activeRect.height === 0) {
        return;
      }

      const x = activeRect.left - containerRect.left;
      const y = activeRect.top - containerRect.top;
      const width = activeRect.width;
      const height = activeRect.height;

      setSliderStyle({
        transform: `translate3d(${x}px, ${y}px, 0)`,
        width: `${width}px`,
        height: `${height}px`,
        opacity: 1,
      });

      setHasMeasured(true);
    },
    [animated]
  );

  useLayoutEffect(() => {
    updatePosition(activeTab);
  }, [updatePosition, activeTab]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || typeof ResizeObserver === 'undefined') return;

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      if (
        lastContainerSize.current &&
        lastContainerSize.current.width === width &&
        lastContainerSize.current.height === height
      ) {
        return;
      }
      lastContainerSize.current = { width, height };
      updatePosition(activeTab);
    });

    observer.observe(container);
    return () => observer.disconnect();
  }, [updatePosition, activeTab]);

  const isSegmented = variant === 'segmented';
  const isThreeColumns = columns === 3 || columns === 'three-columns';

  const isSmall = size === 'sm';
  const isRadioGroup = role === 'radiogroup';

  const containerClasses = isSegmented
    ? [
        'settings-subtab-nav',
        isSmall ? 'is-sm' : '',
        disabled ? 'is-disabled' : '',
        hasMeasured ? 'has-slider' : '',
        className,
      ]
        .filter(Boolean)
        .join(' ')
    : [
        'settings-scenario-cards',
        isThreeColumns ? 'three-columns' : '',
        bordered ? 'is-bordered' : '',
        disabled ? 'is-disabled' : '',
        hasMeasured ? 'has-slider' : '',
        className,
      ]
        .filter(Boolean)
        .join(' ');

  return (
    <div
      ref={containerRef}
      id={id}
      className={containerClasses}
      role={role}
      aria-label={ariaLabel}
      onKeyDown={handleKeyDown}
      style={style}
      data-tooltip={tooltip}
      data-tooltip-pos={tooltip ? tooltipPos : undefined}
      data-tooltip-multiline={tooltipMultiline ? true : undefined}
      tabIndex={disabled && tooltip ? 0 : undefined}
    >
      {animated && hasMeasured && (
        <div
          className={isSegmented ? 'settings-subtab-slider' : 'settings-scenario-cards-slider'}
          style={sliderStyle}
          aria-hidden="true"
        />
      )}
      {items.map((tab) => {
        const buttonId = tab.id || (idPrefix ? `${idPrefix}-${tab.value}` : undefined);
        const panelId = tab.panelId || (getPanelId ? getPanelId(tab.value) : undefined);
        const isSelected = activeTab === tab.value;
        const ariaLabelText =
          tab.ariaLabel || (typeof tab.label === 'string' ? tab.label : undefined);

        const buttonClasses = isSegmented
          ? [
              'settings-subtab-btn',
              isSelected ? (isRadioGroup ? 'active is-active' : 'active') : '',
              tab.className,
            ]
              .filter(Boolean)
              .join(' ')
          : ['settings-scenario-card', isSelected ? 'active' : '', tab.className]
              .filter(Boolean)
              .join(' ');

        return (
          <button
            key={tab.value}
            data-tab-value={tab.value}
            ref={registerButtonRef(tab.value)}
            id={buttonId}
            type="button"
            role={isRadioGroup ? 'radio' : 'tab'}
            aria-selected={isRadioGroup ? undefined : isSelected}
            aria-checked={isRadioGroup ? isSelected : undefined}
            aria-controls={panelId}
            aria-label={ariaLabelText}
            data-tooltip={tab.tooltip}
            data-tooltip-pos={tab.tooltip ? tab.tooltipPos || 'top' : undefined}
            tabIndex={isSelected ? 0 : -1}
            className={buttonClasses}
            onClick={() => onChange(tab.value)}
            disabled={disabled || tab.disabled}
          >
            {isSegmented ? (
              <>
                {tab.icon && <span className="settings-subtab-icon">{tab.icon}</span>}
                <span className="settings-subtab-label">{tab.label}</span>
              </>
            ) : (
              <>
                {tab.icon && <span className="settings-scenario-card-icon">{tab.icon}</span>}
                <span className="settings-scenario-card-text">
                  <span className="settings-scenario-card-label">{tab.label}</span>
                  {tab.description && (
                    <span className="settings-scenario-card-description">{tab.description}</span>
                  )}
                </span>
              </>
            )}
          </button>
        );
      })}
    </div>
  );
}

/** Card-style secondary tabs (e.g. scenario selection, feature models). */
export function ScenarioCardTabs<T extends string = string>(
  props: Omit<SecondaryTabsProps<T>, 'variant'>
): React.JSX.Element {
  return <SecondaryTabs<T> variant="card" {...props} />;
}

/** Segmented pill-style secondary tabs (e.g. prompts sub-tabs, filter pills). */
export function SegmentedSubTabs<T extends string = string>(
  props: Omit<SecondaryTabsProps<T>, 'variant'>
): React.JSX.Element {
  return <SecondaryTabs<T> variant="segmented" {...props} />;
}

export type ScenarioCardTabItem<T extends string = string> = SecondaryTabItem<T>;
export type ScenarioCardTabsProps<T extends string = string> = Omit<
  SecondaryTabsProps<T>,
  'variant'
>;

export type SegmentedSubTabItem<T extends string = string> = SecondaryTabItem<T>;
export type SegmentedSubTabsProps<T extends string = string> = Omit<
  SecondaryTabsProps<T>,
  'variant'
>;
