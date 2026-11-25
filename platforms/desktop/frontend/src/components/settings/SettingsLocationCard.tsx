import { ExternalLink, FolderOpen, RotateCcw } from 'lucide-react';
import type React from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { Tooltip } from '../ui/Tooltip';
import './SettingsShared.css';

export interface SettingsLocationCardProps {
  /** Optional element ID for search scrolling/highlighting */
  id?: string;
  /** Test ID attribute for the card container */
  testId?: string;
  /** Title of the location setting */
  title: string;
  /** Description or hint text displayed under the title */
  hint?: string;
  /** Current file or directory path string */
  path?: string | null;
  /** Whether the location is currently customized */
  isCustom?: boolean;
  /** Label for the custom badge (defaults to common.custom) */
  customBadgeLabel?: string;
  /** Label for the default badge (defaults to common.default) */
  defaultBadgeLabel?: string;
  /** Optional validity status (true = ready, false = missing, undefined/null = no status badge) */
  isValid?: boolean | null;
  /** Label when status is valid (defaults to common.ready) */
  validStatusLabel?: string;
  /** Label when status is invalid (defaults to common.not_found) */
  invalidStatusLabel?: string;
  /** Extra badge elements to render in the title row */
  extraBadges?: ReactNode;
  /** Whether an action is in progress (disables buttons) */
  isBusy?: boolean;
  /** Label for the change/browse button */
  changeLabel?: string;
  /** Callback when change/browse button is clicked */
  onChangePath?: () => void | Promise<void>;
  /** Label for the open folder button */
  openFolderLabel?: string;
  /** Callback when open folder button is clicked */
  onOpenFolder?: () => void | Promise<void>;
  /** Label for the restore default button */
  restoreDefaultLabel?: string;
  /** Callback when restore default button is clicked */
  onRestoreDefault?: () => void | Promise<void>;
  /** Bottom hint text or element (e.g. dynamic status feedback or warning) */
  bottomHint?: ReactNode;
  /** Color override for bottom hint */
  bottomHintColor?: string;
  /** Extra action buttons */
  extraActions?: ReactNode;
}

export function SettingsLocationCard({
  id,
  testId,
  title,
  hint,
  path,
  isCustom = false,
  customBadgeLabel,
  defaultBadgeLabel,
  isValid,
  validStatusLabel,
  invalidStatusLabel,
  extraBadges,
  isBusy = false,
  changeLabel,
  onChangePath,
  openFolderLabel,
  onOpenFolder,
  restoreDefaultLabel,
  onRestoreDefault,
  bottomHint,
  bottomHintColor,
  extraActions,
}: SettingsLocationCardProps): React.JSX.Element {
  const { t } = useTranslation();

  const resolvedCustomBadge = customBadgeLabel ?? t('common.custom', { defaultValue: 'Custom' });
  const resolvedDefaultBadge =
    defaultBadgeLabel ?? t('common.default', { defaultValue: 'Default' });
  const resolvedValidStatus = validStatusLabel ?? t('common.ready', { defaultValue: 'Ready' });
  const resolvedInvalidStatus =
    invalidStatusLabel ?? t('common.not_found', { defaultValue: 'Not Found' });
  const resolvedChangeLabel = changeLabel ?? t('common.browse', { defaultValue: 'Browse...' });
  const resolvedOpenFolderLabel =
    openFolderLabel ?? t('common.open_folder', { defaultValue: 'Open Folder' });
  const resolvedRestoreDefaultLabel =
    restoreDefaultLabel ?? t('common.restore_default', { defaultValue: 'Restore Default' });

  return (
    <div id={id} className="settings-storage-location-card" data-testid={testId}>
      <div className="settings-storage-location-header">
        <div className="settings-storage-location-title-row">
          <span className="settings-storage-location-title">{title}</span>
          <div style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
            <span className={`settings-storage-location-badge ${isCustom ? 'custom' : 'default'}`}>
              {isCustom ? resolvedCustomBadge : resolvedDefaultBadge}
            </span>
            {isValid !== undefined && isValid !== null && (
              <span
                className="settings-storage-location-badge"
                style={{
                  background: isValid
                    ? 'var(--color-success-bg, rgba(16, 185, 129, 0.12))'
                    : 'var(--color-danger-bg, rgba(239, 68, 68, 0.12))',
                  color: isValid ? 'var(--color-success, #10b981)' : 'var(--color-danger, #ef4444)',
                  border: `1px solid ${
                    isValid
                      ? 'var(--color-success-border, rgba(16, 185, 129, 0.25))'
                      : 'var(--color-danger-border, rgba(239, 68, 68, 0.25))'
                  }`,
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '6px',
                }}
              >
                <span
                  style={{
                    width: '6px',
                    height: '6px',
                    borderRadius: '50%',
                    backgroundColor: isValid
                      ? 'var(--color-success, #10b981)'
                      : 'var(--color-danger, #ef4444)',
                  }}
                />
                {isValid ? resolvedValidStatus : resolvedInvalidStatus}
              </span>
            )}
            {extraBadges}
          </div>
        </div>
        {hint && <p className="settings-storage-location-hint">{hint}</p>}
      </div>

      <Tooltip content={path || undefined} placement="top" multiline disabled={!path}>
        <div className="settings-storage-path-box">
          <code>{path || t('common.loading', { defaultValue: 'Loading...' })}</code>
        </div>
      </Tooltip>

      <div className="settings-storage-location-actions">
        {onChangePath && (
          <Button
            tier="secondary"
            size="sm"
            leftIcon={<FolderOpen size={14} />}
            onClick={() => {
              void onChangePath();
            }}
            disabled={isBusy}
          >
            {resolvedChangeLabel}
          </Button>
        )}
        {onOpenFolder && (
          <Button
            tier="ghost"
            size="sm"
            leftIcon={<ExternalLink size={14} />}
            onClick={() => {
              void onOpenFolder();
            }}
            disabled={!path || isBusy}
          >
            {resolvedOpenFolderLabel}
          </Button>
        )}
        {isCustom && onRestoreDefault && (
          <Button
            tier="ghost"
            size="sm"
            leftIcon={<RotateCcw size={14} />}
            onClick={() => {
              void onRestoreDefault();
            }}
            disabled={isBusy}
          >
            {resolvedRestoreDefaultLabel}
          </Button>
        )}
        {extraActions}
      </div>

      {bottomHint && (
        <p
          className="settings-storage-location-hint"
          style={{
            color: bottomHintColor ?? 'var(--color-text-muted)',
            marginTop: '2px',
          }}
        >
          {bottomHint}
        </p>
      )}
    </div>
  );
}

export default SettingsLocationCard;
