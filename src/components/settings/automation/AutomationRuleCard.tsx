import React from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
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
    const chipColors = {
        neutral: {
            background: 'var(--color-bg-secondary)',
            color: 'var(--color-text-muted)',
        },
        warning: {
            background: 'var(--color-warning-bg, rgba(245, 158, 11, 0.12))',
            color: 'var(--color-warning-text, #b7791f)',
        },
        danger: {
            background: 'var(--color-danger-bg, rgba(239, 68, 68, 0.12))',
            color: 'var(--color-danger-text, #b91c1c)',
        },
        success: {
            background: 'var(--color-success-bg, rgba(16, 185, 129, 0.12))',
            color: 'var(--color-success-text, #047857)',
        },
    } as const;

    return (
        <span
            style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                padding: '2px 8px',
                borderRadius: '999px',
                fontSize: '0.75rem',
                fontWeight: 500,
                lineHeight: 1.4,
                ...chipColors[tone],
            }}
        >
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
    onRetryFailed?: () => void;
    onScanNow?: () => void;
    onToggleEnabled?: (value: boolean) => void;
    onToggleExpand: () => void;
    outputDirectory: string;
    pendingCount?: number;
    processingCount?: number;
    projectLabel: string;
    resultLabel?: string;
    resultMessage?: string;
    statusLabel?: string;
    title: string;
    watchDirectory: string;
};

export function AutomationRuleCard({
    blockedHint,
    canToggle,
    editor,
    enabled,
    failureCount,
    isExpanded,
    onDelete,
    onRetryFailed,
    onScanNow,
    onToggleEnabled,
    onToggleExpand,
    outputDirectory,
    pendingCount,
    processingCount,
    projectLabel,
    resultLabel,
    resultMessage,
    statusLabel,
    title,
    watchDirectory,
}: Props): React.JSX.Element {
    const { t } = useTranslation();
    const failureChipTone = (failureCount || 0) > 0 ? 'danger' : 'neutral';
    const resultChipTone = resultLabel === t('automation.last_result_success', { defaultValue: 'Success' })
        ? 'success'
        : resultLabel === t('automation.last_result_error', { defaultValue: 'Failed' })
            ? 'danger'
            : 'neutral';

    return (
        <div
            style={{
                borderBottom: '1px solid var(--color-border-subtle)',
                background: enabled ? 'var(--color-bg-primary)' : 'var(--color-bg-secondary-soft)',
            }}
        >
            <div
                style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: '16px',
                    padding: '16px 24px',
                }}
            >
                <button
                    type="button"
                    onClick={onToggleExpand}
                    aria-expanded={isExpanded}
                    style={{
                        flex: 1,
                        minWidth: 0,
                        display: 'flex',
                        alignItems: 'flex-start',
                        gap: '12px',
                        background: 'transparent',
                        border: 'none',
                        padding: 0,
                        textAlign: 'left',
                        cursor: 'pointer',
                        color: 'inherit',
                    }}
                >
                    <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-muted)', paddingTop: '2px' }}>
                        {isExpanded ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                    </div>

                    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', minWidth: 0, flex: 1 }}>
                        <div className="settings-item-title">{title}</div>

                        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                            <SummaryChip label={projectLabel} tone="neutral" />
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

                        <div className="settings-item-hint" style={{ wordBreak: 'break-all' }}>
                            {t('automation.watch_directory', { defaultValue: 'Watch Directory' })}: {watchDirectory || t('automation.none', { defaultValue: 'None' })}
                        </div>
                        <div className="settings-item-hint" style={{ wordBreak: 'break-all' }}>
                            {t('automation.output_directory', { defaultValue: 'Output Directory' })}: {outputDirectory || t('automation.none', { defaultValue: 'None' })}
                        </div>
                        {resultMessage && (
                            <div className="settings-item-hint" style={{ wordBreak: 'break-word' }}>
                                {resultMessage}
                            </div>
                        )}
                        {blockedHint && (
                            <div
                                className="settings-item-hint"
                                style={{
                                    wordBreak: 'break-word',
                                    color: 'var(--color-warning-text, #b7791f)',
                                }}
                            >
                                {blockedHint}
                            </div>
                        )}
                    </div>
                </button>

                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap', justifyContent: 'flex-end' }}>
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
                        <button className="btn btn-secondary" onClick={onScanNow}>
                            <PlayIcon />
                            <span>{t('automation.scan_now', { defaultValue: 'Scan Now' })}</span>
                        </button>
                    )}

                    {onRetryFailed && (
                        <button className="btn btn-secondary" onClick={onRetryFailed} disabled={!failureCount}>
                            <PauseIcon />
                            <span>{t('automation.retry_failed', { defaultValue: 'Retry Failed' })}</span>
                        </button>
                    )}

                    {onDelete && (
                        <button className="btn btn-secondary" onClick={onDelete}>
                            <TrashIcon />
                            <span>{t('common.delete')}</span>
                        </button>
                    )}
                </div>
            </div>

            {isExpanded && editor}
        </div>
    );
}
