import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BellIcon, RestoreIcon } from './Icons';
import { useRecoveryStore } from '../stores/recoveryStore';

interface NotificationCenterProps {
    onOpenRecoveryCenter: () => void;
}

interface NotificationEntry {
    id: 'recovery';
    kind: 'recovery';
    title: string;
    body: string;
    actionLabel: string;
}

export function NotificationCenter({
    onOpenRecoveryCenter,
}: NotificationCenterProps): React.JSX.Element {
    const { t } = useTranslation();
    const items = useRecoveryStore((state) => state.items);
    const isLoaded = useRecoveryStore((state) => state.isLoaded);
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);
    const panelId = useId();

    const pendingItems = useMemo(
        () => items.filter((item) => item.resolution === 'pending'),
        [items]
    );

    const notifications = useMemo<NotificationEntry[]>(() => {
        if (!isLoaded || pendingItems.length === 0) {
            return [];
        }

        const batchCount = pendingItems.filter((item) => item.source === 'batch_import').length;
        const automationCount = pendingItems.filter((item) => item.source === 'automation').length;

        return [{
            id: 'recovery',
            kind: 'recovery',
            title: t('recovery.banner.title'),
            body: t('recovery.banner.body', {
                count: pendingItems.length,
                batchCount,
                automationCount,
            }),
            actionLabel: t('recovery.actions.open_center'),
        }];
    }, [isLoaded, pendingItems, t]);

    useEffect(() => {
        if (!isOpen) {
            return undefined;
        }

        const handleMouseDown = (event: MouseEvent) => {
            if (!containerRef.current?.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };

        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                setIsOpen(false);
            }
        };

        document.addEventListener('mousedown', handleMouseDown);
        document.addEventListener('keydown', handleKeyDown);

        return () => {
            document.removeEventListener('mousedown', handleMouseDown);
            document.removeEventListener('keydown', handleKeyDown);
        };
    }, [isOpen]);

    const openRecoveryCenter = () => {
        setIsOpen(false);
        onOpenRecoveryCenter();
    };

    return (
        <div className="notification-center" ref={containerRef}>
            <button
                type="button"
                className="btn btn-icon notification-center-trigger"
                onClick={() => setIsOpen((open) => !open)}
                data-tooltip={t('header.notifications')}
                data-tooltip-pos="bottom-left"
                aria-label={t('header.notifications')}
                aria-haspopup="dialog"
                aria-expanded={isOpen}
                aria-controls={panelId}
            >
                <BellIcon />
                {notifications.length > 0 ? (
                    <span className="notification-center-trigger-badge" aria-hidden="true">
                        {notifications.length}
                    </span>
                ) : null}
            </button>

            {isOpen ? (
                <div
                    id={panelId}
                    className="notification-center-panel"
                    role="dialog"
                    aria-label={t('header.notifications_panel')}
                >
                    <div className="notification-center-panel-header">
                        <div className="notification-center-panel-title">
                            {t('header.notifications_panel')}
                        </div>
                    </div>

                    {notifications.length === 0 ? (
                        <div className="notification-center-empty">
                            {t('header.notifications_empty')}
                        </div>
                    ) : (
                        <ul className="notification-center-list">
                            {notifications.map((notification) => (
                                <li key={notification.id} className="notification-center-item">
                                    <button
                                        type="button"
                                        className="notification-center-item-main"
                                        onClick={openRecoveryCenter}
                                    >
                                        <span className="notification-center-item-icon" aria-hidden="true">
                                            {notification.kind === 'recovery' ? <RestoreIcon /> : null}
                                        </span>
                                        <span className="notification-center-item-copy">
                                            <strong>{notification.title}</strong>
                                            <span>{notification.body}</span>
                                        </span>
                                    </button>
                                    <button
                                        type="button"
                                        className="btn btn-secondary notification-center-item-action"
                                        onClick={openRecoveryCenter}
                                    >
                                        {notification.actionLabel}
                                    </button>
                                </li>
                            ))}
                        </ul>
                    )}
                </div>
            ) : null}
        </div>
    );
}
