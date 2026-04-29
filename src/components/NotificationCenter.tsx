import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BellIcon, CheckIcon, CloseIcon, DownloadIcon, RestoreIcon } from './Icons';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useAppUpdater } from '../hooks/useAppUpdater';
import type { UpdateStatus } from '../stores/appUpdaterStore';

interface NotificationCenterProps {
    onOpenRecoveryCenter: () => void;
}

interface RecoveryNotificationEntry {
    id: 'recovery';
    kind: 'recovery';
    title: string;
    body: string;
    actionLabel: string;
}

interface UpdateNotificationEntry {
    id: 'update';
    kind: 'update';
    title: string;
    body: string | null;
    actionLabel: string;
    version: string;
    progress: number;
    status: UpdateStatus;
    isBusy: boolean;
}

type NotificationEntry = RecoveryNotificationEntry | UpdateNotificationEntry;

export function NotificationCenter({
    onOpenRecoveryCenter,
}: NotificationCenterProps): React.JSX.Element {
    const { t } = useTranslation();
    const items = useRecoveryStore((state) => state.items);
    const isLoaded = useRecoveryStore((state) => state.isLoaded);
    const {
        status,
        updateInfo,
        progress,
        notificationVisible,
        installUpdate,
        dismissNotification,
        relaunchToUpdate,
    } = useAppUpdater();
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);
    const panelId = useId();

    const pendingItems = useMemo(
        () => items.filter((item) => item.resolution === 'pending'),
        [items]
    );

    const notifications = useMemo<NotificationEntry[]>(() => {
        const nextNotifications: NotificationEntry[] = [];

        if (notificationVisible && updateInfo) {
            let body: string | null;
            if (status === 'available') {
                body = updateInfo.body || t('settings.update_desc_default');
            } else {
                body = null;
            }

            let actionLabel: string;
            if (status === 'downloaded') {
                actionLabel = t('settings.update_btn_relaunch');
            } else if (status === 'downloading') {
                actionLabel = t('settings.update_downloading');
            } else if (status === 'installing') {
                actionLabel = t('settings.update_installing');
            } else {
                actionLabel = t('settings.update_btn_install');
            }

            nextNotifications.push({
                id: 'update',
                kind: 'update',
                title: t('settings.update_available', { version: updateInfo.version }),
                body,
                actionLabel,
                version: updateInfo.version,
                progress,
                status,
                isBusy: status === 'downloading' || status === 'installing',
            });
        }

        if (isLoaded && pendingItems.length > 0) {
            const batchCount = pendingItems.filter((item) => item.source === 'batch_import').length;
            const automationCount = pendingItems.filter((item) => item.source === 'automation').length;

            nextNotifications.push({
                id: 'recovery',
                kind: 'recovery',
                title: t('recovery.banner.title'),
                body: t('recovery.banner.body', {
                    count: pendingItems.length,
                    batchCount,
                    automationCount,
                }),
                actionLabel: t('recovery.actions.open_center'),
            });
        }

        return nextNotifications;
    }, [isLoaded, notificationVisible, pendingItems, progress, status, t, updateInfo]);

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

    const handleUpdateAction = () => {
        if (status === 'downloaded') {
            void relaunchToUpdate();
            return;
        }

        void installUpdate();
    };

    const renderUpdateNotification = (notification: UpdateNotificationEntry) => (
        <li key={notification.id} className="notification-center-item notification-center-item-update">
            <div className="notification-center-item-header">
                <div className="notification-center-item-main notification-center-item-main-static">
                    <span className="notification-center-item-icon" aria-hidden="true">
                        <DownloadIcon />
                    </span>
                    <span className="notification-center-item-copy">
                        <strong>{notification.title}</strong>
                        {notification.body ? (
                            <span className="notification-center-update-body">
                                {notification.body}
                            </span>
                        ) : null}
                    </span>
                </div>
                <button
                    type="button"
                    className="btn btn-icon notification-center-item-close"
                    onClick={dismissNotification}
                    aria-label={t('common.close')}
                    disabled={notification.isBusy}
                >
                    <CloseIcon />
                </button>
            </div>

            {notification.status === 'downloading' || notification.status === 'installing' ? (
                <div className="update-progress-container notification-center-update-progress">
                    <div className="update-progress-header">
                        <span>
                            {notification.status === 'downloading'
                                ? t('settings.update_downloading')
                                : t('settings.update_installing')}
                        </span>
                        <span>{notification.progress}%</span>
                    </div>
                    <div className="progress-bar">
                        <div className="progress-bar-fill" style={{ width: `${notification.progress}%` }} />
                    </div>
                </div>
            ) : null}

            {notification.status === 'downloaded' ? (
                <div className="update-status success notification-center-update-status">
                    <CheckIcon />
                    <span>{t('settings.update_relaunch')}</span>
                </div>
            ) : null}

            <div className="notification-center-item-actions">
                <button
                    type="button"
                    className="btn btn-primary notification-center-item-action"
                    onClick={handleUpdateAction}
                    disabled={notification.isBusy}
                >
                    {notification.actionLabel}
                </button>
            </div>
        </li>
    );

    const renderRecoveryNotification = (notification: RecoveryNotificationEntry) => (
        <li key={notification.id} className="notification-center-item">
            <button
                type="button"
                className="notification-center-item-main"
                onClick={openRecoveryCenter}
            >
                <span className="notification-center-item-icon" aria-hidden="true">
                    <RestoreIcon />
                </span>
                <span className="notification-center-item-copy">
                    <strong>{notification.title}</strong>
                    <span>{notification.body}</span>
                </span>
            </button>
            <div className="notification-center-item-actions">
                <button
                    type="button"
                    className="btn btn-secondary notification-center-item-action"
                    onClick={openRecoveryCenter}
                >
                    {notification.actionLabel}
                </button>
            </div>
        </li>
    );

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
                                notification.kind === 'update'
                                    ? renderUpdateNotification(notification)
                                    : renderRecoveryNotification(notification)
                            ))}
                        </ul>
                    )}
                </div>
            ) : null}
        </div>
    );
}
