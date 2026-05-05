import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
    AutomationIcon,
    BellIcon,
    CheckIcon,
    CloseIcon,
    CompleteIcon,
    DownloadIcon,
    ErrorIcon,
    FileTextIcon,
    PendingIcon,
    ProcessingIcon,
    RestoreIcon,
    SparklesIcon,
} from './Icons';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useAutomationStore } from '../stores/automationStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import { useAppUpdater } from '../hooks/useAppUpdater';
import type { UpdateStatus } from '../stores/appUpdaterStore';
import type { RecoveryItemStage } from '../types/recovery';
import type { TaskLedgerKind, TaskLedgerRecord, TaskLedgerStatus } from '../types/taskLedger';
import {
    isTaskLedgerActionableStatus,
    isTaskLedgerActiveStatus,
} from '../types/taskLedger';

interface NotificationCenterProps {
    onOpenRecoveryCenter: () => void;
    onOpenAutomationSettings: () => void;
}

type TaskCenterSection = 'needsAction' | 'active' | 'recent';

interface LedgerTaskEntry {
    source: 'ledger';
    id: string;
    section: TaskCenterSection;
    task: TaskLedgerRecord;
}

interface UpdateTaskEntry {
    source: 'update';
    id: 'update';
    section: Exclude<TaskCenterSection, 'recent'>;
    title: string;
    body: string | null;
    actionLabel: string;
    version: string;
    progress: number;
    status: UpdateStatus;
    isBusy: boolean;
}

interface AutomationNotificationEntry {
    source: 'automationNotification';
    id: string;
    section: 'needsAction' | 'recent';
    notificationId: string;
    kind: 'automationFailure' | 'automationSuccess';
    title: string;
    body: string;
    detail: string | null;
    message: string | null;
    retryable: boolean;
    actionLabel: string;
}

type TaskCenterEntry = LedgerTaskEntry | UpdateTaskEntry | AutomationNotificationEntry;
type NotificationTone = 'update' | 'recovery' | 'automation-failure' | 'automation-success' | 'task-active' | 'task-recent';

function getFileName(filePath?: string): string | null {
    if (!filePath) {
        return null;
    }

    const filename = filePath.split(/[/\\]/).pop();
    return filename || null;
}

function getStageLabel(
    stage: RecoveryItemStage | string | undefined,
    t: (key: string, options?: Record<string, unknown>) => string,
): string | null {
    if (!stage) {
        return null;
    }

    return t(`recovery.stage.${stage}`, { defaultValue: stage });
}

function getNotificationBadgeLabel(count: number): string {
    return count > 9 ? '9+' : String(count);
}

function getTaskSection(task: TaskLedgerRecord): TaskCenterSection {
    if (isTaskLedgerActiveStatus(task.status)) {
        return 'active';
    }

    if (isTaskLedgerActionableStatus(task.status)) {
        return 'needsAction';
    }

    return 'recent';
}

function getTaskKindLabel(
    kind: TaskLedgerKind,
    t: (key: string, options?: Record<string, unknown>) => string,
): string {
    const labels: Record<TaskLedgerKind, string> = {
        batchImport: 'Batch import',
        automation: 'Automation',
        llmPolish: 'LLM polish',
        llmTranslate: 'Translation',
        llmSummary: 'AI summary',
        recovery: 'Recovery',
        update: 'Update',
    };
    return t(`task_center.kind.${kind}`, { defaultValue: labels[kind] });
}

function getTaskStatusLabel(
    status: TaskLedgerStatus,
    t: (key: string, options?: Record<string, unknown>) => string,
): string {
    const labels: Record<TaskLedgerStatus, string> = {
        pending: 'Pending',
        running: 'Running',
        cancelRequested: 'Stopping',
        failed: 'Failed',
        recoverable: 'Recoverable',
        interrupted: 'Interrupted',
        cancelled: 'Cancelled',
        succeeded: 'Succeeded',
    };
    return t(`task_center.status.${status}`, { defaultValue: labels[status] });
}

function getTaskTone(task: TaskLedgerRecord): NotificationTone {
    if (task.kind === 'recovery') {
        return 'recovery';
    }

    if (task.status === 'failed' || task.status === 'interrupted') {
        return 'automation-failure';
    }

    if (task.status === 'succeeded') {
        return 'automation-success';
    }

    if (task.status === 'cancelled') {
        return 'task-recent';
    }

    return 'task-active';
}

function getTaskIcon(task: TaskLedgerRecord): React.ReactNode {
    if (task.status === 'failed' || task.status === 'interrupted') {
        return <ErrorIcon />;
    }

    if (task.status === 'succeeded') {
        return <CompleteIcon />;
    }

    if (task.status === 'cancelled') {
        return <CloseIcon />;
    }

    switch (task.kind) {
        case 'automation':
            return <AutomationIcon />;
        case 'llmPolish':
        case 'llmTranslate':
        case 'llmSummary':
            return <SparklesIcon />;
        case 'recovery':
            return <RestoreIcon />;
        case 'batchImport':
            return <FileTextIcon />;
        case 'update':
            return <DownloadIcon />;
        default:
            return task.status === 'running' ? <ProcessingIcon /> : <PendingIcon />;
    }
}

function getRecoveryIdFromTask(taskId: string): string {
    return taskId.startsWith('recovery-') ? taskId.slice('recovery-'.length) : taskId;
}

function getTaskBody(
    task: TaskLedgerRecord,
    t: (key: string, options?: Record<string, unknown>) => string,
): string {
    const kindLabel = getTaskKindLabel(task.kind, t);
    const statusLabel = getTaskStatusLabel(task.status, t);
    const stageLabel = getStageLabel(task.stage, t);
    const fileName = getFileName(task.filePath);
    const base = stageLabel
        ? t('task_center.task_body_stage', {
            defaultValue: '{{kind}} · {{status}} · {{stage}}',
            kind: kindLabel,
            status: statusLabel,
            stage: stageLabel,
        })
        : t('task_center.task_body', {
            defaultValue: '{{kind}} · {{status}}',
            kind: kindLabel,
            status: statusLabel,
        });

    if (!fileName || fileName === task.title) {
        return base;
    }

    return t('task_center.task_body_file', {
        defaultValue: '{{base}} · {{fileName}}',
        base,
        fileName,
    });
}

function NotificationCard({
    itemClassName,
    tone,
    icon,
    title,
    body,
    bodyClassName,
    onOpen,
    closeButton,
    support,
    actions,
}: {
    itemClassName?: string;
    tone: NotificationTone;
    icon: React.ReactNode;
    title: string;
    body?: string | null;
    bodyClassName?: string;
    onOpen?: () => void;
    closeButton?: React.ReactNode;
    support?: React.ReactNode;
    actions?: React.ReactNode;
}): React.JSX.Element {
    const mainClassName = onOpen
        ? 'notification-center-item-main'
        : 'notification-center-item-main notification-center-item-main-static';

    const copy = (
        <>
            <span className="notification-center-item-icon" aria-hidden="true">
                {icon}
            </span>
            <span className="notification-center-item-copy">
                <strong className="notification-center-item-title">{title}</strong>
                {body ? (
                    <span className={`notification-center-item-body${bodyClassName ? ` ${bodyClassName}` : ''}`}>
                        {body}
                    </span>
                ) : null}
            </span>
        </>
    );

    return (
        <li className={`notification-center-item notification-center-item-tone-${tone}${itemClassName ? ` ${itemClassName}` : ''}`}>
            <div className="notification-center-item-header">
                {onOpen ? (
                    <button
                        type="button"
                        className={mainClassName}
                        onClick={onOpen}
                    >
                        {copy}
                    </button>
                ) : (
                    <div className={mainClassName}>
                        {copy}
                    </div>
                )}
                {closeButton}
            </div>
            {support ? (
                <div className="notification-center-item-support">
                    {support}
                </div>
            ) : null}
            {actions ? (
                <div className="notification-center-item-actions">
                    {actions}
                </div>
            ) : null}
        </li>
    );
}

export function NotificationCenter({
    onOpenRecoveryCenter,
    onOpenAutomationSettings,
}: NotificationCenterProps): React.JSX.Element {
    const { t } = useTranslation();
    const tasks = useTaskLedgerStore((state) => state.tasks);
    const requestTaskCancel = useTaskLedgerStore((state) => state.requestCancel);
    const removeTask = useTaskLedgerStore((state) => state.removeTask);
    const clearResolvedTasks = useTaskLedgerStore((state) => state.clearResolved);
    const resumeRecoveryItem = useRecoveryStore((state) => state.resumeItem);
    const discardRecoveryItem = useRecoveryStore((state) => state.discardItem);
    const retryAutomationRule = useAutomationStore((state) => state.retryFailed);
    const automationNotifications = useAutomationStore((state) => state.notifications);
    const dismissAutomationNotification = useAutomationStore((state) => state.dismissNotification);
    const retryAutomationNotification = useAutomationStore((state) => state.retryNotification);
    const addBatchFiles = useBatchQueueStore((state) => state.addFiles);
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

    const entries = useMemo<TaskCenterEntry[]>(() => {
        const nextEntries: TaskCenterEntry[] = tasks.map((task) => ({
            source: 'ledger',
            id: task.id,
            section: getTaskSection(task),
            task,
        }));

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

            nextEntries.push({
                source: 'update',
                id: 'update',
                section: status === 'downloading' || status === 'installing' ? 'active' : 'needsAction',
                title: t('settings.update_available', { version: updateInfo.version }),
                body,
                actionLabel,
                version: updateInfo.version,
                progress,
                status,
                isBusy: status === 'downloading' || status === 'installing',
            });
        }

        automationNotifications
            .filter((notification) => notification.kind === 'failure')
            .sort((a, b) => b.updatedAt - a.updatedAt)
            .forEach((notification) => {
                const latestFileName = getFileName(notification.latestFilePath)
                    || t('automation.notifications.file_unknown');
                const stageLabel = getStageLabel(notification.latestStage, t);

                nextEntries.push({
                    source: 'automationNotification',
                    id: notification.id,
                    section: 'needsAction',
                    notificationId: notification.id,
                    kind: 'automationFailure',
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
                });
            });

        automationNotifications
            .filter((notification) => notification.kind === 'success')
            .sort((a, b) => b.updatedAt - a.updatedAt)
            .forEach((notification) => {
                const latestFileName = getFileName(notification.latestFilePath)
                    || t('automation.notifications.file_unknown');
                const stageLabel = getStageLabel(notification.latestStage, t);

                nextEntries.push({
                    source: 'automationNotification',
                    id: notification.id,
                    section: 'recent',
                    notificationId: notification.id,
                    kind: 'automationSuccess',
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
                    message: null,
                    actionLabel: t('automation.open_settings', { defaultValue: 'Open Automation' }),
                    retryable: false,
                });
            });

        return nextEntries.sort((a, b) => {
            const aUpdatedAt = a.source === 'ledger' ? a.task.updatedAt : a.source === 'update' ? Number.MAX_SAFE_INTEGER : 0;
            const bUpdatedAt = b.source === 'ledger' ? b.task.updatedAt : b.source === 'update' ? Number.MAX_SAFE_INTEGER : 0;
            return bUpdatedAt - aUpdatedAt;
        });
    }, [
        automationNotifications,
        notificationVisible,
        progress,
        status,
        t,
        tasks,
        updateInfo,
    ]);

    const groupedEntries = useMemo(() => ({
        needsAction: entries.filter((entry) => entry.section === 'needsAction'),
        active: entries.filter((entry) => entry.section === 'active'),
        recent: entries.filter((entry) => entry.section === 'recent'),
    }), [entries]);

    const badgeCount = groupedEntries.needsAction.length + groupedEntries.active.length;
    const notificationBadgeLabel = useMemo(
        () => getNotificationBadgeLabel(badgeCount),
        [badgeCount],
    );

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

    const handleRetryTask = (task: TaskLedgerRecord) => {
        if (task.kind === 'automation' && task.automationRuleId) {
            void retryAutomationRule(task.automationRuleId);
            void removeTask(task.id);
            return;
        }

        if (task.kind === 'batchImport' && task.filePath) {
            addBatchFiles([task.filePath], { projectId: task.projectId ?? null });
            void removeTask(task.id);
        }
    };

    const renderProgress = (progressValue: number) => (
        <div className="update-progress-container notification-center-update-progress">
            <div className="update-progress-header">
                <span>{t('task_center.progress', { defaultValue: 'Progress' })}</span>
                <span>{Math.round(progressValue)}%</span>
            </div>
            <div className="progress-bar">
                <div className="progress-bar-fill" style={{ width: `${Math.max(0, Math.min(progressValue, 100))}%` }} />
            </div>
        </div>
    );

    const renderUpdateEntry = (entry: UpdateTaskEntry) => {
        const support = (entry.status === 'downloading' || entry.status === 'installing' || entry.status === 'downloaded')
            ? (
                <>
                    {entry.status === 'downloading' || entry.status === 'installing' ? renderProgress(entry.progress) : null}
                    {entry.status === 'downloaded' ? (
                        <div className="update-status success notification-center-update-status">
                            <CheckIcon />
                            <span>{t('settings.update_relaunch')}</span>
                        </div>
                    ) : null}
                </>
            )
            : null;

        return (
            <NotificationCard
                key={entry.id}
                tone="update"
                itemClassName="notification-center-item-update"
                icon={<DownloadIcon />}
                title={entry.title}
                body={entry.body}
                bodyClassName="notification-center-update-body"
                closeButton={(
                    <button
                        type="button"
                        className="btn btn-icon notification-center-item-close"
                        onClick={dismissUpdateNotification}
                        aria-label={t('common.close')}
                        disabled={entry.isBusy}
                    >
                        <CloseIcon />
                    </button>
                )}
                support={support}
                actions={(
                    <button
                        type="button"
                        className="btn btn-primary btn-sm notification-center-item-action"
                        onClick={handleUpdateAction}
                        disabled={entry.isBusy}
                    >
                        {entry.actionLabel}
                    </button>
                )}
            />
        );
    };

    const renderLedgerTaskActions = (task: TaskLedgerRecord): React.ReactNode => {
        if (task.kind === 'recovery' && task.status === 'recoverable') {
            const recoveryId = getRecoveryIdFromTask(task.id);
            return (
                <>
                    <button
                        type="button"
                        className="btn btn-primary btn-sm notification-center-item-action"
                        onClick={() => void resumeRecoveryItem(recoveryId)}
                        disabled={!task.recoverable}
                    >
                        {t('common.resume', { defaultValue: 'Resume' })}
                    </button>
                    <button
                        type="button"
                        className="btn btn-secondary-soft btn-sm notification-center-item-action"
                        onClick={() => void discardRecoveryItem(recoveryId)}
                    >
                        {t('task_center.discard', { defaultValue: 'Discard' })}
                    </button>
                    <button
                        type="button"
                        className="btn btn-secondary btn-sm notification-center-item-action"
                        onClick={openRecoveryCenter}
                    >
                        {t('recovery.actions.open_center')}
                    </button>
                </>
            );
        }

        if (isTaskLedgerActiveStatus(task.status)) {
            return (
                <button
                    type="button"
                    className="btn btn-secondary-soft btn-sm notification-center-item-action"
                    onClick={() => void requestTaskCancel(task.id)}
                    disabled={!task.cancelable || task.status === 'cancelRequested'}
                >
                    {task.status === 'cancelRequested'
                        ? t('task_center.stopping', { defaultValue: 'Stopping' })
                        : t('common.cancel')}
                </button>
            );
        }

        if (isTaskLedgerActionableStatus(task.status)) {
            const canRetry = (
                (task.kind === 'automation' && !!task.automationRuleId)
                || (task.kind === 'batchImport' && !!task.filePath)
            );

            return (
                <>
                    {canRetry ? (
                        <button
                            type="button"
                            className="btn btn-primary btn-sm notification-center-item-action"
                            onClick={() => handleRetryTask(task)}
                        >
                            {t('task_center.retry', { defaultValue: 'Retry' })}
                        </button>
                    ) : null}
                    {task.kind === 'automation' && !canRetry ? (
                        <button
                            type="button"
                            className="btn btn-secondary btn-sm notification-center-item-action"
                            onClick={openAutomationSettings}
                        >
                            {t('automation.open_settings', { defaultValue: 'Open Automation' })}
                        </button>
                    ) : null}
                    <button
                        type="button"
                        className="btn btn-secondary-soft btn-sm notification-center-item-action"
                        onClick={() => void removeTask(task.id)}
                    >
                        {t('task_center.dismiss', { defaultValue: 'Dismiss' })}
                    </button>
                </>
            );
        }

        return (
            <button
                type="button"
                className="btn btn-secondary-soft btn-sm notification-center-item-action"
                onClick={() => void removeTask(task.id)}
            >
                {t('task_center.clear', { defaultValue: 'Clear' })}
            </button>
        );
    };

    const renderLedgerTask = (entry: LedgerTaskEntry) => {
        const { task } = entry;
        const stageLabel = getStageLabel(task.stage, t);
        const hasSupport = isTaskLedgerActiveStatus(task.status) || Boolean(stageLabel) || Boolean(task.errorMessage);
        const support = hasSupport ? (
            <>
                {isTaskLedgerActiveStatus(task.status) ? renderProgress(task.progress) : null}
                {stageLabel && !isTaskLedgerActiveStatus(task.status) ? (
                    <div className="notification-center-item-detail">
                        {t('automation.notifications.stage_detail', { stage: stageLabel })}
                    </div>
                ) : null}
                {task.errorMessage ? (
                    <div className="notification-center-item-message">
                        {task.errorMessage}
                    </div>
                ) : null}
            </>
        ) : null;

        return (
            <NotificationCard
                key={entry.id}
                tone={getTaskTone(task)}
                itemClassName="notification-center-item-task"
                icon={getTaskIcon(task)}
                title={task.title}
                body={getTaskBody(task, t)}
                support={support}
                actions={renderLedgerTaskActions(task)}
            />
        );
    };

    const renderAutomationNotification = (entry: AutomationNotificationEntry) => {
        const isFailure = entry.kind === 'automationFailure';
        const actionClassName = isFailure && entry.retryable
            ? 'btn btn-primary btn-sm notification-center-item-action'
            : 'btn btn-secondary btn-sm notification-center-item-action';
        const support = entry.detail || (isFailure && entry.message)
            ? (
                <>
                    {entry.detail ? (
                        <div className="notification-center-item-detail">
                            {entry.detail}
                        </div>
                    ) : null}
                    {isFailure && entry.message ? (
                        <div className="notification-center-item-message">
                            {entry.message}
                        </div>
                    ) : null}
                </>
            )
            : null;

        return (
            <NotificationCard
                key={entry.id}
                tone={isFailure ? 'automation-failure' : 'automation-success'}
                itemClassName={isFailure
                    ? 'notification-center-item-automation-failure'
                    : 'notification-center-item-automation-success'}
                icon={isFailure ? <ErrorIcon /> : <AutomationIcon />}
                title={entry.title}
                body={entry.body}
                onOpen={openAutomationSettings}
                closeButton={(
                    <button
                        type="button"
                        className="btn btn-icon notification-center-item-close"
                        onClick={() => dismissAutomationNotification(entry.notificationId)}
                        aria-label={t('common.close')}
                    >
                        <CloseIcon />
                    </button>
                )}
                support={support}
                actions={(
                    <button
                        type="button"
                        className={actionClassName}
                        onClick={() => {
                            if (isFailure && entry.retryable) {
                                void retryAutomationNotification(entry.notificationId);
                                return;
                            }
                            openAutomationSettings();
                        }}
                    >
                        {entry.actionLabel}
                    </button>
                )}
            />
        );
    };

    const renderEntry = (entry: TaskCenterEntry) => {
        if (entry.source === 'update') {
            return renderUpdateEntry(entry);
        }

        if (entry.source === 'automationNotification') {
            return renderAutomationNotification(entry);
        }

        return renderLedgerTask(entry);
    };

    const renderSection = (
        section: TaskCenterSection,
        title: string,
        sectionEntries: TaskCenterEntry[],
    ) => {
        if (sectionEntries.length === 0) {
            return null;
        }

        return (
            <section className={`notification-center-section notification-center-section-${section}`}>
                <div className="notification-center-section-title">
                    {title}
                    <span>{sectionEntries.length}</span>
                </div>
                <ul className="notification-center-list">
                    {sectionEntries.map(renderEntry)}
                </ul>
            </section>
        );
    };

    const hasEntries = entries.length > 0;

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
                {badgeCount > 0 ? (
                    <span className="notification-center-trigger-badge" aria-hidden="true">
                        {notificationBadgeLabel}
                    </span>
                ) : null}
            </button>

            {isOpen ? (
                <div
                    id={panelId}
                    className="notification-center-panel"
                    role="dialog"
                    aria-label={t('task_center.panel_title', { defaultValue: 'Task Center' })}
                >
                    <div className="notification-center-panel-header">
                        <div className="notification-center-panel-title">
                            {t('task_center.panel_title', { defaultValue: 'Task Center' })}
                        </div>
                        {groupedEntries.recent.length > 0 ? (
                            <button
                                type="button"
                                className="btn btn-secondary-soft btn-sm notification-center-clear-resolved"
                                onClick={() => void clearResolvedTasks()}
                            >
                                {t('task_center.clear_recent', { defaultValue: 'Clear recent' })}
                            </button>
                        ) : null}
                    </div>

                    {!hasEntries ? (
                        <div className="notification-center-empty">
                            {t('task_center.empty', { defaultValue: 'No active tasks right now.' })}
                        </div>
                    ) : (
                        <>
                            {renderSection('needsAction', t('task_center.needs_action', { defaultValue: 'Needs action' }), groupedEntries.needsAction)}
                            {renderSection('active', t('task_center.active', { defaultValue: 'Active' }), groupedEntries.active)}
                            {renderSection('recent', t('task_center.recent', { defaultValue: 'Recent' }), groupedEntries.recent)}
                        </>
                    )}
                </div>
            ) : null}
        </div>
    );
}
