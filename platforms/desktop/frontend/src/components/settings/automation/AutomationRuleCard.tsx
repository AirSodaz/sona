import React from 'react';
import { ChevronDown, ChevronRight, ArrowRight, Sparkles, FolderSync, Tags, HardDriveDownload } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { PauseIcon, PlayIcon, TrashIcon } from '../../Icons';
import { Switch } from '../../Switch';

function SummaryChip({
    label,
    tone = 'neutral',
}: {
    label: string;
    tone?: 'neutral' | 'warning' | 'danger' | 'success';
}): React.JSX.Element {
    const toneClass = `automation-chip automation-chip-${tone}`;
    return (
        <span className={toneClass}>
            {label}
        </span>
    );
}

type Props = {
    blockedHint?: string | null;
    canToggle: boolean;
    editor?: React.JSX.Element | null;
    enabled: boolean;
    failureCount?: number;
    isExpanded: boolean;
    onDelete?: () => void;
    onApplyExisting?: () => void;
    onRetryFailed?: () => void;
    onScanNow?: () => void;
    onToggleEnabled?: (value: boolean) => void;
    onToggleExpand: () => void;
    outputDirectory?: string;
    pendingCount?: number;
    processingCount?: number;
    projectLabel: string;
    profileLabel?: string;
    priorityLabel?: string;
    migrationNotice?: string;
    typeLabel?: string;
    resultLabel?: string;
    resultMessage?: string;
    statusLabel?: string;
    title: string;
    watchDirectory?: string;
};

export function AutomationRuleCard({
    blockedHint,
    canToggle,
    editor,
    enabled,
    failureCount,
    isExpanded,
    onDelete,
    onApplyExisting,
    onRetryFailed,
    onScanNow,
    onToggleEnabled,
    onToggleExpand,
    outputDirectory,
    pendingCount,
    processingCount,
    projectLabel,
    profileLabel,
    priorityLabel,
    migrationNotice,
    resultLabel,
    resultMessage,
    statusLabel,
    title,
    typeLabel,
    watchDirectory,
}: Props): React.JSX.Element {
    const { t } = useTranslation();
    const failureChipTone = (failureCount || 0) > 0 ? 'danger' : 'neutral';
    const resultChipTone = resultLabel === t('automation.last_result_success', { defaultValue: 'Success' })
        ? 'success'
        : resultLabel === t('automation.last_result_error', { defaultValue: 'Failed' })
            ? 'danger'
            : 'neutral';

    const isFileRule = typeLabel === t('automation.file_rule', { defaultValue: 'File' }) || watchDirectory !== undefined;
    const isWatching = enabled && (statusLabel === t('automation.status_watching', { defaultValue: 'Watching' }) || statusLabel === 'Watching');
    const isError = (failureCount || 0) > 0 || statusLabel === 'Error' || statusLabel === t('automation.status_error', { defaultValue: 'Error' });

    let statusDotClass = 'automation-status-dot';
    if (isWatching) statusDotClass += ' dot-watching';
    else if (isError) statusDotClass += ' dot-error';

    return (
        <div className={`automation-rule-wrapper ${enabled ? 'is-enabled' : 'is-disabled'}`}>
            {/* Main Header Bar */}
            <div className="automation-rule-card-header">
                <button
                    type="button"
                    onClick={onToggleExpand}
                    aria-expanded={isExpanded}
                    className="automation-rule-card-main"
                >
                    <div className="automation-card-title-line">
                        <span className={statusDotClass} />
                        <span className="automation-card-title-text settings-item-title">{title}</span>
                        {typeLabel && (
                            <span className="automation-card-type-tag">
                                {isFileRule ? <FolderSync size={12} /> : <Tags size={12} />}
                                <span>{typeLabel}</span>
                            </span>
                        )}
                        <span className="automation-rule-chevron">
                            {isExpanded ? <ChevronDown size={18} /> : <ChevronRight size={18} />}
                        </span>
                    </div>

                    {/* Flowchart Summary */}
                    <div className="automation-card-flow-body">
                        {isFileRule ? (
                            <span className="automation-flow-node-box node-source" title={watchDirectory}>
                                <FolderSync size={13} />
                                <span>{watchDirectory ? (watchDirectory.split(/[/\\]/).pop() || watchDirectory) : t('automation.none', { defaultValue: 'None' })}</span>
                            </span>
                        ) : (
                            <span className="automation-flow-node-box node-source" title={projectLabel}>
                                <Tags size={13} />
                                <span>{projectLabel}</span>
                            </span>
                        )}

                        <span className="automation-flow-arrow-icon"><ArrowRight size={13} /></span>

                        <span className="automation-flow-node-box node-ai" title={profileLabel || t('automation.profile_global_fallback', { defaultValue: 'Global settings' })}>
                            <Sparkles size={13} />
                            <span>{profileLabel || t('automation.profile_global_fallback', { defaultValue: 'Global settings' })}</span>
                        </span>

                        {isFileRule && outputDirectory && (
                            <>
                                <span className="automation-flow-arrow-icon"><ArrowRight size={13} /></span>
                                <span className="automation-flow-node-box node-export" title={outputDirectory}>
                                    <HardDriveDownload size={13} />
                                    <span>{outputDirectory.split(/[/\\]/).pop() || outputDirectory}</span>
                                </span>
                            </>
                        )}

                        <div style={{ marginLeft: 'auto', display: 'flex', gap: '6px', alignItems: 'center' }}>
                            {priorityLabel && <SummaryChip label={priorityLabel} tone="neutral" />}
                            {statusLabel && <SummaryChip label={statusLabel} tone="neutral" />}
                            {resultLabel && <SummaryChip label={resultLabel} tone={resultChipTone} />}
                            {typeof failureCount === 'number' && (
                                <SummaryChip
                                    label={t('automation.failure_count', {
                                        defaultValue: '{{count}} failures',
                                        count: failureCount,
                                    })}
                                    tone={failureChipTone}
                                />
                            )}
                            {!!pendingCount && (
                                <SummaryChip
                                    label={t('automation.pending_count', {
                                        defaultValue: '{{count}} pending',
                                        count: pendingCount,
                                    })}
                                    tone="neutral"
                                />
                            )}
                            {!!processingCount && (
                                <SummaryChip
                                    label={t('automation.processing_count', {
                                        defaultValue: '{{count}} processing',
                                        count: processingCount,
                                    })}
                                    tone="warning"
                                />
                            )}
                        </div>
                    </div>

                    {/* Metadata & Hints */}
                    <div className="automation-card-meta-row">
                        {watchDirectory !== undefined && (
                            <div className="settings-item-hint">
                                {t('automation.watch_directory', { defaultValue: 'Watch Directory' })}: {watchDirectory || t('automation.none', { defaultValue: 'None' })}
                            </div>
                        )}
                        {outputDirectory !== undefined && (
                            <div className="settings-item-hint">
                                {t('automation.output_directory', { defaultValue: 'Output Directory' })}: {outputDirectory || t('automation.none', { defaultValue: 'None' })}
                            </div>
                        )}
                        {resultMessage && (
                            <div className="settings-item-hint">
                                {resultMessage}
                            </div>
                        )}
                        {blockedHint && (
                            <div className="settings-item-hint" style={{ color: 'var(--color-warning-text, #b7791f)' }}>
                                {blockedHint}
                            </div>
                        )}
                        {migrationNotice && (
                            <div className="settings-item-hint" style={{ color: 'var(--color-warning-text, #b7791f)' }}>
                                {migrationNotice}
                            </div>
                        )}
                    </div>
                </button>

                <div className="automation-card-controls">
                    {canToggle && onToggleEnabled && (
                        <Switch
                            checked={enabled}
                            onChange={onToggleEnabled}
                            aria-label={t('automation.toggle_rule', {
                                defaultValue: 'Enable {{name}}',
                                name: title,
                            })}
                        />
                    )}

                    {onScanNow && (
                        <button className="btn btn-secondary" onClick={onScanNow} title={t('automation.scan_now', { defaultValue: 'Scan Now' })}>
                            <PlayIcon />
                            <span>{t('automation.scan_now', { defaultValue: 'Scan Now' })}</span>
                        </button>
                    )}

                    {onRetryFailed && (
                        <button className="btn btn-secondary" onClick={onRetryFailed} disabled={!failureCount} title={t('automation.retry_failed', { defaultValue: 'Retry Failed' })}>
                            <PauseIcon />
                            <span>{t('automation.retry_failed', { defaultValue: 'Retry Failed' })}</span>
                        </button>
                    )}

                    {onApplyExisting && (
                        <button className="btn btn-secondary" onClick={onApplyExisting} title={t('automation.apply_existing', { defaultValue: 'Apply to existing' })}>
                            <PlayIcon />
                            <span>{t('automation.apply_existing', { defaultValue: 'Apply to existing' })}</span>
                        </button>
                    )}

                    {onDelete && (
                        <button className="btn btn-secondary" onClick={onDelete} title={t('common.delete')}>
                            <TrashIcon />
                            <span>{t('common.delete')}</span>
                        </button>
                    )}
                </div>
            </div>

            {/* Expandable Step Editor */}
            {isExpanded && editor}
        </div>
    );
}

export default AutomationRuleCard;
