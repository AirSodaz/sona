import React from 'react';
import {
  AlertCircle,
  BarChart3,
  Bot,
  Clock3,
  FileText,
  LoaderCircle,
} from 'lucide-react';
import type {
  DashboardOverviewStats,
  DashboardLlmUsageStats,
  DashboardSpeakerStats,
} from '../../../types/dashboard';
import { type DashboardTranslation } from './formatters';
import {
  KpiCard,
  SpeakerOverviewCard,
  SpeakerRankingCard,
  StatPill,
  StatusBadge,
  UsageBreakdown,
} from './cards';
import { ContentTrends, TokenTrend } from './trend';

export function ContentOverviewSection({
  overview,
  isDeepLoading,
  error,
  t,
}: {
  overview: DashboardOverviewStats;
  isDeepLoading: boolean;
  error: string | null;
  t: DashboardTranslation;
}): React.JSX.Element {
  const itemSparkline = overview.recentDailyItems.map((point) => ({
    label: point.dateLabel,
    value: point.itemCount,
  }));
  const durationSparkline = overview.recentDailyItems.map((point) => ({
    label: point.dateLabel,
    value: point.durationSeconds,
  }));

  return (
    <div className="settings-dashboard-subsection">
      <div className="settings-dashboard-subsection-header">
        <div className="settings-dashboard-subtitle-stack">
          <div className="settings-dashboard-subtitle">{t('settings.dashboard.content_overview', { defaultValue: 'Content Overview' })}</div>
        </div>
        {(isDeepLoading || error) && (
          <div className="settings-dashboard-status-rail">
            {isDeepLoading && (
              <StatusBadge
                icon={<LoaderCircle className="spin" size={14} />}
                label={t('settings.dashboard.partial_loading', { defaultValue: 'Loading transcript and speaker details...' })}
                testId="dashboard-partial"
              />
            )}
            {error && (
              <StatusBadge
                icon={<AlertCircle size={14} />}
                label={t('settings.dashboard.partial_error', {
                  defaultValue: 'Some dashboard details could not be refreshed: {{message}}',
                  message: error,
                })}
                tone="warning"
              />
            )}
          </div>
        )}
      </div>

      <div className="settings-dashboard-feature-grid">
        <KpiCard
          label={t('settings.dashboard.items', { defaultValue: 'Items' })}
          value={overview.itemCountDisplay}
          badge={<FileText size={16} />}
          variant="feature"
          tone="accent"
          sparkline={itemSparkline}
          sparklineLabel={t('settings.dashboard.recent_item_trend', { defaultValue: 'Recent 30 Day Item Trend' })}
          detail={(
            <div className="settings-dashboard-pill-row">
              <StatPill>
                {t('settings.dashboard.recording_pill', {
                  defaultValue: '{{count}} recording',
                  count: overview.recordingCountDisplay,
                })}
              </StatPill>
              <StatPill>
                {t('settings.dashboard.batch_pill', {
                  defaultValue: '{{count}} batch',
                  count: overview.batchCountDisplay,
                })}
              </StatPill>
            </div>
          )}
        />
        <KpiCard
          label={t('settings.dashboard.total_duration', { defaultValue: 'Total Duration' })}
          value={overview.totalDurationDisplay}
          badge={<Clock3 size={16} />}
          variant="feature"
          tone="warm"
          sparkline={durationSparkline}
          sparklineLabel={t('settings.dashboard.recent_duration_trend', { defaultValue: 'Recent 30 Day Duration Trend' })}
        />
      </div>

      <div className="settings-dashboard-support-grid">
        <KpiCard
          label={t('settings.dashboard.transcript_characters', { defaultValue: 'Transcript Characters' })}
          value={typeof overview.transcriptCharacterCount === 'number'
            ? overview.transcriptCharacterCountDisplay || ''
            : t('settings.dashboard.scanning', { defaultValue: 'Scanning...' })}
          muted={typeof overview.transcriptCharacterCount !== 'number'}
          detail={typeof overview.transcriptCharacterCount !== 'number'
            ? t('settings.dashboard.partial_loading', { defaultValue: 'Loading transcript and speaker details...' })
            : undefined}
        />
        <KpiCard
          label={t('settings.dashboard.tags', { defaultValue: 'Tags' })}
          value={overview.tagCountDisplay}
          detail={(
            <div className="settings-dashboard-pill-row">
              <StatPill>
                {t('settings.dashboard.untagged_pill', {
                  defaultValue: '{{count}} untagged',
                  count: overview.untaggedCountDisplay,
                })}
              </StatPill>
              <StatPill>
                {t('settings.dashboard.tagged_pill', {
                  defaultValue: '{{count}} tagged',
                  count: overview.taggedCountDisplay,
                })}
              </StatPill>
            </div>
          )}
        />
      </div>

      <ContentTrends points={overview.recentDailyItems} t={t} />
    </div>
  );
}

export function SpeakerInsightsSection({
  speakers,
  isDeepLoading,
  error,
  t,
}: {
  speakers: DashboardSpeakerStats | null;
  isDeepLoading: boolean;
  error: string | null;
  t: DashboardTranslation;
}): React.JSX.Element {
  const statusMessage = isDeepLoading
    ? t('settings.dashboard.deep_scan_loading', { defaultValue: 'Speaker stats are still scanning saved transcripts.' })
    : error;

  return (
    <div className="settings-dashboard-subsection">
      <div className="settings-dashboard-subsection-header">
        <div className="settings-dashboard-subtitle-stack">
          <div className="settings-dashboard-subtitle">{t('settings.dashboard.speaker_insights', { defaultValue: 'Speaker Insights' })}</div>
        </div>
        {isDeepLoading && (
          <div className="settings-dashboard-status-rail">
            <StatusBadge
              icon={<LoaderCircle className="spin" size={14} />}
              label={t('settings.dashboard.scanning', { defaultValue: 'Scanning...' })}
              testId="dashboard-speaker-loading"
            />
          </div>
        )}
      </div>

      <div className="settings-dashboard-speaker-kpi-grid">
        <KpiCard
          label={t('settings.dashboard.annotated_items', { defaultValue: 'Speaker-Annotated Items' })}
          value={speakers ? speakers.annotatedItemCountDisplay : '...'}
          muted={!speakers}
          compact
        />
        <KpiCard
          label={t('settings.dashboard.speaker_attributed_duration', { defaultValue: 'Speaker-Attributed Duration' })}
          value={speakers ? speakers.speakerAttributedDurationDisplay : '...'}
          muted={!speakers}
          compact
        />
        <KpiCard
          label={t('settings.dashboard.identified_speakers', { defaultValue: 'Identified Speakers' })}
          value={speakers ? speakers.identifiedSpeakerCountDisplay : '...'}
          muted={!speakers}
          compact
        />
        <KpiCard
          label={t('settings.dashboard.anonymous_slots', { defaultValue: 'Anonymous Speaker Slots' })}
          value={speakers ? speakers.anonymousSpeakerSlotCountDisplay : '...'}
          muted={!speakers}
          compact
        />
      </div>

      <div className="settings-dashboard-speaker-detail-grid">
        <SpeakerOverviewCard
          speakers={speakers}
          t={t}
          statusMessage={statusMessage}
        />
        <SpeakerRankingCard
          speakers={speakers}
          t={t}
          statusMessage={statusMessage}
        />
      </div>
    </div>
  );
}

export function LlmUsagePanel({
  llmUsage,
  t,
}: {
  llmUsage: DashboardLlmUsageStats;
  t: DashboardTranslation;
}): React.JSX.Element {
  const callSparkline = llmUsage.recentDaily.map((point) => ({
    label: point.dateLabel,
    value: point.callCount,
  }));
  const tokenSparkline = llmUsage.recentDaily.map((point) => ({
    label: point.dateLabel,
    value: point.totalTokens,
  }));

  return (
    <div className="settings-dashboard-panel">
      <div className="settings-dashboard-feature-grid">
        <KpiCard
          label={t('settings.dashboard.llm_call_count', { defaultValue: 'Successful Calls' })}
          value={llmUsage.totals.callCountDisplay}
          badge={<Bot size={16} />}
          variant="feature"
          tone="info"
          sparkline={callSparkline}
          sparklineLabel={t('settings.dashboard.llm_call_count', { defaultValue: 'Successful Calls' })}
        />
        <KpiCard
          label={t('settings.dashboard.total_tokens', { defaultValue: 'Total Tokens' })}
          value={llmUsage.totals.totalTokensDisplay}
          badge={<BarChart3 size={16} />}
          variant="feature"
          tone="accent"
          sparkline={tokenSparkline}
          sparklineLabel={t('settings.dashboard.recent_token_trend', { defaultValue: 'Recent 30 Day Token Trend' })}
        />
      </div>

      <div className="settings-dashboard-support-grid">
        <KpiCard
          label={t('settings.dashboard.calls_with_usage', { defaultValue: 'Calls With Usage' })}
          value={llmUsage.totals.callsWithUsageDisplay}
        />
        <KpiCard
          label={t('settings.dashboard.calls_without_usage', { defaultValue: 'Calls Missing Usage' })}
          value={llmUsage.totals.callsWithoutUsageDisplay}
          tone={llmUsage.totals.callsWithoutUsage > 0 ? 'warm' : 'default'}
        />
      </div>

      <div className="settings-dashboard-meta-rail">
        <div className="settings-dashboard-meta-pill">
          <BarChart3 size={14} />
          <span>
            {t('settings.dashboard.tokens_hint', {
              defaultValue: '{{prompt}} prompt / {{completion}} completion',
              prompt: llmUsage.totals.promptTokensDisplay,
              completion: llmUsage.totals.completionTokensDisplay,
            })}
          </span>
        </div>
        <div className="settings-dashboard-meta-pill">
          <Clock3 size={14} />
          <span>
            {llmUsage.startedAt
              ? t('settings.dashboard.tracking_since', {
                defaultValue: 'Tracking since {{date}}',
                date: llmUsage.trackingSinceDisplay || llmUsage.startedAt,
              })
              : t('settings.dashboard.no_tracked_calls', { defaultValue: 'No tracked calls yet.' })}
          </span>
        </div>
        {llmUsage.totals.callsWithoutUsage > 0 && (
          <div className="settings-dashboard-meta-pill warning">
            <AlertCircle size={14} />
            <span>
              {t('settings.dashboard.missing_usage_hint', {
                defaultValue: '{{count}} successful calls did not include token usage from the provider.',
                count: llmUsage.totals.callsWithoutUsageDisplay,
              })}
            </span>
          </div>
        )}
      </div>

      <div className="settings-dashboard-usage-grid">
        <TokenTrend points={llmUsage.recentDaily} t={t} />
        <UsageBreakdown
          title={t('settings.dashboard.by_provider', { defaultValue: 'By Provider' })}
          breakdown={llmUsage.byProviderTopRows}
          maxValue={llmUsage.byProviderMaxValue}
          t={t}
        />
        <UsageBreakdown
          title={t('settings.dashboard.by_category', { defaultValue: 'By Category' })}
          breakdown={llmUsage.byCategoryTopRows}
          maxValue={llmUsage.byCategoryMaxValue}
          t={t}
        />
      </div>
    </div>
  );
}
