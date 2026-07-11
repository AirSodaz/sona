import React, { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
    AlertTriangle,
    Clock3,
    FileStack,
    Loader2,
    RefreshCw,
    Trash2,
    Workflow,
} from 'lucide-react';
import { useProjectStore } from '../stores/projectStore';
import { useRecoveryStore } from '../stores/recoveryStore';
import type { RecoveryItemStage, RecoverySource, RecoveredQueueItem } from '../types/recovery';
import { PanelModal } from './PanelModal';
import './RecoveryCenterModal.css';

interface RecoveryCenterModalProps {
    isOpen: boolean;
    onClose: () => void;
}

function formatRecoveredAt(updatedAt: number | null, t: (key: string, options?: Record<string, unknown>) => string): string {
    if (!updatedAt) {
        return t('recovery.labels.unknown_time');
    }

    return new Date(updatedAt).toLocaleString();
}

function getStageLabel(stage: RecoveryItemStage, t: (key: string, options?: Record<string, unknown>) => string): string {
    return t(`recovery.stage.${stage}`);
}

function getSourceTitle(source: RecoverySource, t: (key: string, options?: Record<string, unknown>) => string): string {
    return t(`recovery.source.${source}`);
}

function describeRecoveryItem(
    item: RecoveredQueueItem,
    projectName: string | null,
    t: (key: string, options?: Record<string, unknown>) => string,
): string {
    if (!item.canResume) {
        return t('recovery.item.source_missing');
    }

    if (item.source === 'automation') {
        return t('recovery.item.automation_description', {
            ruleName: item.automationRuleName || t('recovery.labels.automation_rule_unknown'),
            projectName: projectName || t('recovery.labels.no_project'),
        });
    }

    return t('recovery.item.batch_description', {
        projectName: projectName || t('recovery.labels.no_project'),
    });
}

export function RecoveryCenterModal({
    isOpen,
    onClose,
}: RecoveryCenterModalProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const projects = useProjectStore((state) => state.projects);
    const items = useRecoveryStore((state) => state.items);
    const updatedAt = useRecoveryStore((state) => state.updatedAt);
    const isLoaded = useRecoveryStore((state) => state.isLoaded);
    const isBusy = useRecoveryStore((state) => state.isBusy);
    const error = useRecoveryStore((state) => state.error);
    const resumeAll = useRecoveryStore((state) => state.resumeAll);
    const resumeItem = useRecoveryStore((state) => state.resumeItem);
    const discardItem = useRecoveryStore((state) => state.discardItem);
    const discardAll = useRecoveryStore((state) => state.discardAll);

    useEffect(() => {
        if (isOpen && isLoaded && items.length === 0 && !isBusy) {
            onClose();
        }
    }, [isBusy, isLoaded, isOpen, items.length, onClose]);

    const grouped = useMemo(() => {
        const batch = items.filter((item) => item.source === 'batch_import');
        const automation = items.filter((item) => item.source === 'automation');
        return { batch, automation };
    }, [items]);

    const hasMissingSources = items.some((item) => !item.canResume);
    const projectNames = useMemo(() => new Map(projects.map((project) => [project.id, project.name])), [projects]);

    const overviewCards = useMemo(() => ([
        {
            source: 'batch_import' as const,
            title: t('recovery.source.batch_import'),
            description: t('recovery.overview.batch_description'),
            count: grouped.batch.length,
            draftCount: grouped.batch.filter((item) => item.historyId || item.segments.length > 0).length,
            icon: <FileStack size={16} />,
        },
        {
            source: 'automation' as const,
            title: t('recovery.source.automation'),
            description: t('recovery.overview.automation_description'),
            count: grouped.automation.length,
            draftCount: grouped.automation.filter((item) => item.historyId || item.segments.length > 0).length,
            icon: <Workflow size={16} />,
        },
    ]), [grouped.automation, grouped.batch, t]);

    const handleResumeAll = async () => {
        try {
            await resumeAll();
        } catch {
            // Store error state drives the banner.
        }
    };

    const handleDiscardAll = async () => {
        try {
            await discardAll();
        } catch {
            // Store error state drives the banner.
        }
    };

    const handleDiscardItem = async (id: string) => {
        try {
            await discardItem(id);
        } catch {
            // Store error state drives the banner.
        }
    };

    const handleResumeItem = async (id: string) => {
        try {
            await resumeItem(id);
        } catch {
            // Store error state drives the banner.
        }
    };

    if (!isOpen) {
        return null;
    }

    return (
        <PanelModal
            isOpen={isOpen}
            onClose={onClose}
            ariaLabelledby="recovery-center-title"
            size="settings"
            className="recovery-modal"
            overlayClassName="recovery-overlay"
            badge={(
                <>
                    <RefreshCw size={16} />
                    <span>{t('recovery.badge')}</span>
                </>
            )}
            title={<h2 id="recovery-center-title">{t('recovery.title')}</h2>}
            description={t('recovery.description')}
            headerActions={(
                <>
                    <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => void handleResumeAll()}
                        disabled={isBusy || items.length === 0 || hasMissingSources}
                    >
                        {isBusy ? <Loader2 size={14} className="queue-icon-spin" /> : <RefreshCw size={14} />}
                        {t('recovery.actions.resume_all')}
                    </button>
                    <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => void handleDiscardAll()}
                        disabled={isBusy || items.length === 0}
                    >
                        <Trash2 size={14} />
                        {t('recovery.actions.discard_all')}
                    </button>
                </>
            )}
            meta={(
                <>
                    <span className="panel-modal-meta-label recovery-meta-label">{t('recovery.labels.last_recovered')}</span>
                    <span>{formatRecoveredAt(updatedAt, t)}</span>
                </>
            )}
            errorBanner={error ? (
                <div className="recovery-error-banner" role="alert">
                    {error}
                </div>
            ) : null}
        >
            <section className="recovery-overview-grid" aria-label={t('recovery.labels.overview')}>
                {overviewCards.map((card) => (
                    <article key={card.source} className="recovery-overview-card">
                        <div className="recovery-overview-metrics">
                            <span className="recovery-pill">
                                {card.icon}
                                {t('recovery.overview.pending_count', { count: card.count })}
                            </span>
                            <span className="recovery-pill">
                                <Clock3 size={14} />
                                {t('recovery.overview.draft_count', { count: card.draftCount })}
                            </span>
                        </div>
                        <div className="recovery-overview-title">{card.title}</div>
                        <div className="recovery-overview-description">{card.description}</div>
                    </article>
                ))}
            </section>

            {(['batch_import', 'automation'] as const).map((source) => {
                const sourceItems = source === 'batch_import' ? grouped.batch : grouped.automation;
                if (sourceItems.length === 0) {
                    return null;
                }

                return (
                    <section key={source} className="panel-modal-section recovery-section">
                        <div className="panel-modal-section-header recovery-section-header">
                            <div className="panel-modal-section-title recovery-section-title">{getSourceTitle(source, t)}</div>
                            <div className="panel-modal-section-description recovery-section-description">
                                {source === 'automation'
                                    ? t('recovery.section.automation_description')
                                    : t('recovery.section.batch_description')}
                            </div>
                        </div>
                        <div className="panel-modal-section-body recovery-section-body">
                            {sourceItems.map((item) => {
                                const projectName = item.projectId ? (projectNames.get(item.projectId) || null) : null;
                                return (
                                    <div key={item.id} className="recovery-item-row">
                                        <div className="recovery-item-main">
                                            <div className="recovery-item-title-row">
                                                <div className="recovery-item-title">{item.filename}</div>
                                            </div>
                                            <div className="recovery-item-meta">
                                                <span className="recovery-meta-pill">
                                                    {getSourceTitle(item.source, t)}
                                                </span>
                                                <span className="recovery-meta-pill">
                                                    {t('recovery.labels.stage')}: {getStageLabel(item.lastKnownStage, t)}
                                                </span>
                                                <span className="recovery-meta-pill">
                                                    {t('recovery.labels.saved_at')}: {new Date(item.updatedAt).toLocaleString()}
                                                </span>
                                                {(item.historyId || item.segments.length > 0) ? (
                                                    <span className="recovery-meta-pill">
                                                        {t('recovery.labels.partial_draft')}
                                                    </span>
                                                ) : null}
                                                {!item.canResume ? (
                                                    <span className="recovery-meta-pill warning">
                                                        <AlertTriangle size={14} />
                                                        {t('recovery.labels.source_missing')}
                                                    </span>
                                                ) : null}
                                            </div>
                                            <div className="recovery-item-description">
                                                {describeRecoveryItem(item, projectName, t)}
                                            </div>
                                        </div>
                                        <div className="recovery-item-actions">
                                            <button
                                                type="button"
                                                className="btn btn-secondary panel-modal-inline-action recovery-inline-action"
                                                onClick={() => void handleResumeItem(item.id)}
                                                disabled={isBusy || !item.canResume}
                                            >
                                                {isBusy ? <Loader2 size={14} className="queue-icon-spin" /> : <RefreshCw size={14} />}
                                                {t('common.resume', { defaultValue: 'Resume' })}
                                            </button>
                                            <button
                                                type="button"
                                                className="btn btn-secondary panel-modal-inline-action recovery-inline-action"
                                                onClick={() => void handleDiscardItem(item.id)}
                                                disabled={isBusy}
                                            >
                                                {isBusy ? <Loader2 size={14} className="queue-icon-spin" /> : <Trash2 size={14} />}
                                                {t('recovery.actions.discard')}
                                            </button>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    </section>
                );
            })}
        </PanelModal>
    );
}
