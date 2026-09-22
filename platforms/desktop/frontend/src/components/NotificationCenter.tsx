import type React from 'react';
import { useEffect, useId, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { getBadgeLabel, useNotificationEntries } from '../hooks/useNotificationEntries';
import { useNotificationPanelStore } from '../stores/notificationPanelStore';
import { BellIcon } from './Icons';
import { NotificationPanel } from './NotificationPanel';

interface NotificationCenterProps {
  onOpenRecoveryCenter: () => void;
  onOpenAutomationSettings: () => void;
}

export function NotificationCenter({
  onOpenRecoveryCenter,
  onOpenAutomationSettings,
}: NotificationCenterProps): React.JSX.Element {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const panelId = useId();

  const isOpen = useNotificationPanelStore((state) => state.isOpen);
  const toggle = useNotificationPanelStore((state) => state.toggle);
  const close = useNotificationPanelStore((state) => state.close);

  const { entries, grouped, badgeCount, succeededCount, totalClearableCount, handleClear } =
    useNotificationEntries({
      onOpenRecoveryCenter,
      onOpenAutomationSettings,
      closePanel: close,
    });

  // Close on outside click
  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleMouseDown = (event: MouseEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) {
        close();
      }
    };

    document.addEventListener('mousedown', handleMouseDown);
    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
    };
  }, [isOpen, close]);

  // Close on Escape
  useEscapeKey(
    () => {
      close();
    },
    {
      enabled: isOpen,
      checkTopMost: true,
      containerRef,
    }
  );

  return (
    <div className="notification-center" ref={containerRef}>
      <button
        type="button"
        className="btn btn-icon notification-center-trigger"
        onClick={toggle}
        data-tooltip={t('header.notifications')}
        data-tooltip-pos="bottom-left"
        aria-label={t('header.notifications')}
        aria-haspopup="dialog"
        aria-expanded={isOpen}
        aria-controls={panelId}
      >
        <BellIcon />
        {badgeCount > 0 ? (
          <span className="notification-center-trigger-badge" aria-hidden="true">
            {getBadgeLabel(badgeCount)}
          </span>
        ) : null}
      </button>

      {isOpen ? (
        <NotificationPanel
          panelId={panelId}
          grouped={grouped}
          hasEntries={entries.length > 0}
          succeededCount={succeededCount}
          totalClearableCount={totalClearableCount}
          onClear={handleClear}
        />
      ) : null}
    </div>
  );
}
