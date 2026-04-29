import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    AutomationIcon,
    BellIcon,
    CheckIcon,
    CloseIcon,
    DownloadIcon,
    ErrorIcon,
    RestoreIcon,
} from './Icons';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useAutomationStore } from '../stores/automationStore';
import { useAppUpdater } from '../hooks/useAppUpdater';
import type { UpdateStatus } from '../stores/appUpdaterStore';
import type { RecoveryItemStage } from '../types/recovery';

interface NotificationCenterProps {
    onOpenRecoveryCenter: () => void;
    onOpenAutomationSettings: () => void;
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

interface AutomationFailureNotificationEntry {
    id: string;
    kind: 'automationFailure';
    notificationId: string;
    title: string;
    body: string;
    detail: string | null;
    message: string | null;
    actionLabel: string;
    retryable: boolean;
}

interface AutomationSuccessNotificationEntry {
    id: string;
    kind: 'automationSuccess';
    notificationId: string;
    title: string;
    body: string;
    detail: string | null;
    actionLabel: string;
    retryable: false;
}

type NotificationEntry =
    | RecoveryNotificationEntry
    | UpdateNotificationEntry
    | AutomationFailureNotificationEntry
    | AutomationSuccessNotificationEntry;

function getFileName(filePath?: string): string | null {
    if (!filePath) {
        return null;
    }

    const filename = filePath.split(/[/\\]/).pop();
    return filename || null;
}

function getStageLabel(
    stage: RecoveryItemStage | undefined,
    t: (key: string, options?: Record<string, unknown>) => string,
): string | null {
    if (!stage) {
        return null;
    }

    return t(`recovery.stage.${stage}`);
}

export function NotificationCenter({
    onOpenRecoveryCenter,
    onOpenAutomationSettings,
}: NotificationCenterProps): React.JSX.Element {
    const { t } = useTranslation();
    const items = useRecoveryStore((state) => state.items);
    const isLoaded = useRecoveryStore((state) => state.isLoaded);
    const automationNotifications = useAutomationStore((state) => state.notifications);
    const dismissAutomationNotification = useAutomationStore((state) => state.dismissNotification);
    const retryAutomationNotification = useAutomationStore((state) => state.retryNotification);
    const {
        status,
        updateInfo,
        progress,
        notificationVisible,
        installUpdate,
        dismissNotification: dismissUpdateNotification,
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

        const failureNotifications = automationNotifications
            .filter((notification) => notification.kind === 'failure')
            .sort((a, b) => b.updatedAt - a.updatedAt)
            .map<AutomationFailureNotificationEntry>((notification) => {
                const latestFileName = getFileName(notification.latestFilePath)
                    || t('automation.notifications.file_unknown');
                const stageLabel = getStageLabel(notification.latestStage, t);

                return {
                    id: notification.id,
                    kind: 'automationFailure',
                    notificationId: notification.id,
                    title: t('automation.notifications.failure_title', {
                        ruleName: notification.ruleName,
                    }),
                    body: t('automation.notifications.failure_body', {
                        count: notification.count,
                        fileName: latestFileName,
                    }),
                    detail: stageLabel
                        ? t('automation.notifications.stage_detail', { stage: stageLabel })
                        : null,
                    message: notification.latestMessage || null,
                    actionLabel: notification.retryable
                        ? t('automation.retry_failed', { defaultValue: 'Retry Failed' })
                        : t('automation.open_settings', { defaultValue: 'Open Automation' }),
                    retryable: notification.retryable,
                };
            });

        nextNotifications.push(...failureNotifications);

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

        const successNotifications = automationNotifications
            .filter((notification) => notification.kind === 'success')
            .sort((a, b) => b.updatedAt - a.updatedAt)
            .map<AutomationSuccessNotificationEntry>((notification) => {
                const latestFileName = getFileName(notification.latestFilePath)
                    || t('automation.notifications.file_unknown');
                const stageLabel = getStageLabel(notification.latestStage, t);

                return {
                    id: notification.id,
                    kind: 'automationSuccess',
                    notificationId: notification.id,
                    title: t('automation.notifications.success_title', {
                        ruleName: notification.ruleName,
                    }),
                    body: t('automation.notifications.success_body', {
                        count: notification.count,
                        fileName: latestFileName,
                    }),
                    detail: stageLabel
                        ? t('automation.notifications.stage_detail', { stage: stageLabel })
                        : null,
                    actionLabel: t('automation.open_settings', { defaultValue: 'Open Automation' }),
                    retryable: false,
                };
            });

        nextNotifications.push(...successNotifications);

        return nextNotifications;
    }, [
        automationNotifications,
        isLoaded,
        notificationVisible,
        pendingItems,
        progress,
        status,
        t,
        updateInfo,
    ]);

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

    const openAutomationSettings = () => {
        setIsOpen(false);
        onOpenAutomationSettings();
    };

    const handleUpdateAction = () => {
        if (status === 'downloaded') {
            void relaunchToUpdate();
            return;
        }

        void installUpdate();
    };

    const handleAutomationAction = (notificationId: string, retryable: boolean) => {
        if (retryable) {
            void retryAutomationNotification(notificationId);
            return;
        }

        openAutomationSettings();
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
                    onClick={dismissUpdateNotification}
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

    const renderAutomationNotification = (
        notification: AutomationFailureNotificationEntry | AutomationSuccessNotificationEntry
    ) => {
        const isFailure = notification.kind === 'automationFailure';
        const actionClassName = isFailure && notification.retryable
            ? 'btn btn-primary notification-center-item-action'
            : 'btn btn-secondary notification-center-item-action';

        return (
            <li
                key={notification.id}
                className={`notification-center-item ${isFailure
                    ? 'notification-center-item-automation-failure'
                    : 'notification-center-item-automation-success'}`}
            >
                <div className="notification-center-item-header">
                    <button
                        type="button"
                        className="notification-center-item-main"
                        onClick={openAutomationSettings}
                    >
                        <span className="notification-center-item-icon" aria-hidden="true">
                            {isFailure ? <ErrorIcon /> : <AutomationIcon />}
                        </span>
                        <span className="notification-center-item-copy">
                            <strong>{notification.title}</strong>
                            <span>{notification.body}</span>
                            {notification.detail ? (
                                <span className="notification-center-item-detail">
                                    {notification.detail}
                                </span>
                            ) : null}
                            {isFailure && notification.message ? (
                                <span className="notification-center-item-message">
                                    {notification.message}
                                </span>
                            ) : null}
                        </span>
                    </button>
                    <button
                        type="button"
                        className="btn btn-icon notification-center-item-close"
                        onClick={() => dismissAutomationNotification(notification.notificationId)}
                        aria-label={t('common.close')}
                    >
                        <CloseIcon />
                    </button>
                </div>
                <div className="notification-center-item-actions">
                    <button
                        type="button"
                        className={actionClassName}
                        onClick={() => handleAutomationAction(
                            notification.notificationId,
                            isFailure ? notification.retryable : false,
                        )}
                    >
                        {notification.actionLabel}
                    </button>
                </div>
            </li>
        );
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
                            {notifications.map((notification) => {
                                if (notification.kind === 'update') {
                                    return renderUpdateNotification(notification);
                                }

                                if (notification.kind === 'recovery') {
                                    return renderRecoveryNotification(notification);
                                }

                                return renderAutomationNotification(notification);
                            })}
                        </ul>
                    )}
                </div>
            ) : null}
        </div>
    );
}
