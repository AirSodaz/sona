import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
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
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import { useAppUpdater } from '../hooks/useAppUpdater';
import { useTaskLedgerActions, type TaskCenterAction } from '../hooks/useTaskLedgerActions';
import type { UpdateStatus } from '../stores/appUpdaterStore';
import type { RecoveryItemStage } from '../types/recovery';
import type { TaskLedgerKind, TaskLedgerRecord, TaskLedgerStatus } from '../types/taskLedger';
import {
    isTaskLedgerActionableStatus,
    isTaskLedgerActiveStatus,
} from '../types/taskLedger';
import { useRecoveryStore } from '../stores/recoveryStore';

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
    version: string;
    progress: number;
    status: UpdateStatus;
    isBusy: boolean;
}

type TaskCenterEntry = LedgerTaskEntry | UpdateTaskEntry;
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

function getStaleQueueTaskIdForRecovery(task: TaskLedgerRecord): string | null {
    if (task.kind !== 'recovery' || task.status !== 'recoverable' || !task.id.startsWith('recovery-')) {
        return null;
    }

    return `batch-${task.id.slice('recovery-'.length)}`;
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

function isLlmTaskKind(kind: TaskLedgerKind): boolean {
    return kind === 'llmPolish' || kind === 'llmTranslate' || kind === 'llmSummary';
}

function shouldHideRecoveryRelatedTaskUntilLoaded(task: TaskLedgerRecord): boolean {
    if (task.kind === 'recovery') {
        return true;
    }

    if (task.kind !== 'batchImport' && task.kind !== 'automation') {
        return false;
    }

    return task.status === 'interrupted'
        || task.status === 'recoverable'
        || isTaskLedgerActiveStatus(task.status);
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
    const clearResolvedTasks = useTaskLedgerStore((state) => state.clearResolved);
    const isRecoveryLoaded = useRecoveryStore((state) => state.isLoaded);
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
    const closePanel = useCallback(() => {
        setIsOpen(false);
    }, []);
    const taskActions = useTaskLedgerActions({
        t,
        onOpenRecoveryCenter,
        onOpenAutomationSettings,
        closePanel,
        updater: {
            installUpdate,
            dismissNotification: dismissUpdateNotification,
            relaunchToUpdate,
        },
    });

    const entries = useMemo<TaskCenterEntry[]>(() => {
        const staleQueueTaskIds = new Set(
            tasks
                .map(getStaleQueueTaskIdForRecovery)
                .filter((id): id is string => Boolean(id)),
        );
        const nextEntries: TaskCenterEntry[] = tasks
            .filter((task) => (
                isRecoveryLoaded || !shouldHideRecoveryRelatedTaskUntilLoaded(task)
            ))
            .filter((task) => !staleQueueTaskIds.has(task.id))
            .map((task) => ({
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

            nextEntries.push({
                source: 'update',
                id: 'update',
                section: status === 'downloading' || status === 'installing' ? 'active' : 'needsAction',
                title: t('settings.update_available', { version: updateInfo.version }),
                body,
                version: updateInfo.version,
                progress,
                status,
                isBusy: status === 'downloading' || status === 'installing',
            });
        }

        return nextEntries.sort((a, b) => {
            const aUpdatedAt = a.source === 'ledger' ? a.task.updatedAt : a.source === 'update' ? Number.MAX_SAFE_INTEGER : 0;
            const bUpdatedAt = b.source === 'ledger' ? b.task.updatedAt : b.source === 'update' ? Number.MAX_SAFE_INTEGER : 0;
            return bUpdatedAt - aUpdatedAt;
        });
    }, [
        notificationVisible,
        progress,
        status,
        t,
        tasks,
        updateInfo,
        isRecoveryLoaded,
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

    const getActionButtonClassName = (action: TaskCenterAction) => {
        const variantClassName = action.variant === 'primary'
            ? 'btn-primary'
            : action.variant === 'secondary'
                ? 'btn-secondary'
                : 'btn-secondary-soft';

        return `btn ${variantClassName} btn-sm notification-center-item-action`;
    };

    const runAction = (action: TaskCenterAction) => {
        if (action.disabled) {
            return;
        }

        void action.run();
    };

    const renderActions = (actions: TaskCenterAction[]): React.ReactNode => {
        if (actions.length === 0) {
            return null;
        }

        return actions.map((action) => (
            <button
                key={action.id}
                type="button"
                className={getActionButtonClassName(action)}
                onClick={() => runAction(action)}
                disabled={action.disabled}
            >
                {action.label}
            </button>
        ));
    };

    const renderCloseAction = (action?: TaskCenterAction): React.ReactNode => {
        if (!action) {
            return null;
        }

        return (
            <button
                type="button"
                className="btn btn-icon notification-center-item-close"
                onClick={() => runAction(action)}
                aria-label={action.label}
                disabled={action.disabled}
            >
                <CloseIcon />
            </button>
        );
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
        const actions = taskActions.getUpdateTaskActions({
            status: entry.status,
            isBusy: entry.isBusy,
        });
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
                closeButton={renderCloseAction(actions.close)}
                support={support}
                actions={renderActions(actions.row)}
            />
        );
    };

    const renderLedgerTaskActions = (task: TaskLedgerRecord): React.ReactNode => (
        renderActions(taskActions.getLedgerTaskActions(task))
    );

    const renderLedgerTask = (entry: LedgerTaskEntry) => {
        const { task } = entry;
        const stageLabel = getStageLabel(task.stage, t);
        const isCancelPendingLlmTask = task.status === 'cancelRequested' && isLlmTaskKind(task.kind);
        const hasSupport = isTaskLedgerActiveStatus(task.status)
            || Boolean(stageLabel)
            || Boolean(task.errorMessage)
            || isCancelPendingLlmTask;
        const support = hasSupport ? (
            <>
                {isTaskLedgerActiveStatus(task.status) ? renderProgress(task.progress) : null}
                {isCancelPendingLlmTask ? (
                    <div className="notification-center-item-detail">
                        {t('task_center.cancel_pending_hint', {
                            defaultValue: 'Stops after the current step and skips the final writeback.',
                        })}
                    </div>
                ) : null}
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

    const renderEntry = (entry: TaskCenterEntry) => {
        if (entry.source === 'update') {
            return renderUpdateEntry(entry);
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
