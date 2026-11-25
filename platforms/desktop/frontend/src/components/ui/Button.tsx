import type React from 'react';
import { Tooltip, type TooltipPlacement } from './Tooltip';

export type ButtonTier = 'primary' | 'secondary' | 'ghost';
export type ButtonTone = 'neutral' | 'danger';
export type ButtonSize = 'xs' | 'sm' | 'md';
export type ButtonFeedback = 'idle' | 'success' | 'error';

export interface ButtonProps extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'title'> {
  tier?: ButtonTier;
  tone?: ButtonTone;
  size?: ButtonSize;
  loading?: boolean;
  feedback?: ButtonFeedback;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
  fullWidth?: boolean;
  pill?: boolean;
  active?: boolean;
  tooltip?: React.ReactNode | false;
  tooltipPos?: TooltipPlacement;
  tooltipMultiline?: boolean;
  ref?: React.Ref<HTMLButtonElement>;
}

export interface IconButtonProps
  extends Omit<ButtonProps, 'leftIcon' | 'rightIcon' | 'fullWidth' | 'children'> {
  icon: React.ReactNode;
  'aria-label': string;
}

function buildButtonClasses({
  tier,
  tone,
  size,
  iconOnly,
  loading,
  feedback,
  fullWidth,
  pill,
  active,
  className,
}: {
  tier: ButtonTier;
  tone: ButtonTone;
  size: ButtonSize;
  iconOnly: boolean;
  loading: boolean;
  feedback: ButtonFeedback;
  fullWidth: boolean;
  pill: boolean;
  active: boolean;
  className?: string;
}): string {
  return [
    'btn',
    `btn-${tier}`,
    `btn-${size}`,
    tone === 'danger' ? 'btn-danger' : '',
    iconOnly ? 'btn-icon' : '',
    pill ? 'btn-pill' : '',
    fullWidth ? 'btn-full' : '',
    active ? 'is-active' : '',
    loading ? 'is-loading' : '',
    feedback !== 'idle' ? `btn-feedback-${feedback}` : '',
    className ?? '',
  ]
    .filter(Boolean)
    .join(' ');
}

function ButtonSpinner(): React.JSX.Element {
  return (
    <span className="btn-spinner" aria-hidden="true">
      <svg
        className="btn-spinner-svg"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
      >
        <path d="M21 12a9 9 0 1 1-6.219-8.56" />
      </svg>
    </span>
  );
}

/**
 * Standard button primitive enforcing Sona's 3-tier × 2-tone visual hierarchy.
 */
export function Button({
  tier = 'secondary',
  tone = 'neutral',
  size = 'md',
  loading = false,
  feedback = 'idle',
  leftIcon,
  rightIcon,
  fullWidth = false,
  pill = false,
  active = false,
  tooltip,
  tooltipPos = 'top',
  tooltipMultiline = false,
  type = 'button',
  disabled = false,
  className,
  children,
  ref,
  ...rest
}: ButtonProps): React.JSX.Element {
  const isDisabled = Boolean(disabled || loading);
  const classes = buildButtonClasses({
    tier,
    tone,
    size,
    iconOnly: false,
    loading,
    feedback,
    fullWidth,
    pill,
    active,
    className,
  });

  const buttonEl = (
    <button
      ref={ref}
      type={type}
      className={classes}
      disabled={isDisabled}
      aria-busy={loading || undefined}
      {...rest}
    >
      <span className="btn-content">
        {leftIcon && (
          <span className="btn-content-icon" aria-hidden="true">
            {leftIcon}
          </span>
        )}
        {children}
        {rightIcon && (
          <span className="btn-content-icon" aria-hidden="true">
            {rightIcon}
          </span>
        )}
      </span>
      {loading && <ButtonSpinner />}
    </button>
  );

  return (
    <Tooltip
      content={tooltip}
      placement={tooltipPos}
      multiline={tooltipMultiline}
      disabled={tooltip === undefined || tooltip === false}
      triggerDisabled={isDisabled && tooltip !== undefined && tooltip !== false}
      fullWidthWrapper={fullWidth}
    >
      {buttonEl}
    </Tooltip>
  );
}

/**
 * Square icon-only button primitive requiring an explicit accessible name (`aria-label`).
 * Automatically renders a unified Portal Tooltip reusing `aria-label` unless `tooltip={false}`
 * or a custom `tooltip` is provided.
 */
export function IconButton({
  tier = 'ghost',
  tone = 'neutral',
  size = 'sm',
  loading = false,
  feedback = 'idle',
  pill = false,
  active = false,
  tooltip,
  tooltipPos = 'top',
  tooltipMultiline = false,
  type = 'button',
  disabled = false,
  className,
  icon,
  ref,
  ...rest
}: IconButtonProps): React.JSX.Element {
  const isDisabled = Boolean(disabled || loading);
  const classes = buildButtonClasses({
    tier,
    tone,
    size,
    iconOnly: true,
    loading,
    feedback,
    fullWidth: false,
    pill,
    active,
    className,
  });

  const resolvedTooltip = tooltip === undefined ? rest['aria-label'] : tooltip;

  const buttonEl = (
    <button
      ref={ref}
      type={type}
      className={classes}
      disabled={isDisabled}
      aria-busy={loading || undefined}
      {...rest}
    >
      <span className="btn-content" aria-hidden="true">
        {icon}
      </span>
      {loading && <ButtonSpinner />}
    </button>
  );

  return (
    <Tooltip
      content={resolvedTooltip}
      placement={tooltipPos}
      multiline={tooltipMultiline}
      disabled={resolvedTooltip === false || resolvedTooltip === undefined}
      triggerDisabled={isDisabled && resolvedTooltip !== false && resolvedTooltip !== undefined}
    >
      {buttonEl}
    </Tooltip>
  );
}
