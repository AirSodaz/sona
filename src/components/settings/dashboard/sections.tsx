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
import {
  calculateCoverage,
  formatDuration,
  formatNumber,
  type DashboardTranslation,
} from './formatters';
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
          value={formatNumber(overview.itemCount)}
          badge={<FileText size={16} />}
          variant="feature"
          tone="accent"
          detail={(
            <div className="settings-dashboard-pill-row">
              <StatPill>
                {t('settings.dashboard.recording_pill', {
                  defaultValue: '{{count}} recording',
                  count: formatNumber(overview.recordingCount),
                })}
              </StatPill>
              <StatPill>
                {t('settings.dashboard.batch_pill', {
                  defaultValue: '{{count}} batch',
                  count: formatNumber(overview.batchCount),
                })}
              </StatPill>
            </div>
          )}
        />
        <KpiCard
          label={t('settings.dashboard.total_duration', { defaultValue: 'Total Duration' })}
          value={formatDuration(overview.totalDurationSeconds, t)}
          badge={<Clock3 size={16} />}
          variant="feature"
          tone="warm"
        />
      </div>

      <div className="settings-dashboard-support-grid">
        <KpiCard
          label={t('settings.dashboard.transcript_characters', { defaultValue: 'Transcript Characters' })}
          value={typeof overview.transcriptCharacterCount === 'number'
            ? formatNumber(overview.transcriptCharacterCount)
            : t('settings.dashboard.scanning', { defaultValue: 'Scanning...' })}
          muted={typeof overview.transcriptCharacterCount !== 'number'}
          detail={typeof overview.transcriptCharacterCount !== 'number'
            ? t('settings.dashboard.partial_loading', { defaultValue: 'Loading transcript and speaker details...' })
            : undefined}
        />
        <KpiCard
          label={t('settings.dashboard.projects', { defaultValue: 'Projects' })}
          value={formatNumber(overview.projectCount)}
          detail={(
            <div className="settings-dashboard-pill-row">
              <StatPill>
                {t('settings.dashboard.inbox_pill', {
                  defaultValue: '{{count}} in Inbox',
                  count: formatNumber(overview.inboxCount),
                })}
              </StatPill>
              <StatPill>
                {t('settings.dashboard.project_pill', {
                  defaultValue: '{{count}} in projects',
                  count: formatNumber(overview.projectAssignedCount),
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
  const segmentCoverage = speakers
    ? calculateCoverage(speakers.speakerTaggedSegmentCount, speakers.totalSegmentCount)
    : 0;
  const durationCoverage = speakers
    ? calculateCoverage(speakers.speakerAttributedDuration, speakers.totalSegmentDuration)
    : 0;
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
          value={speakers ? formatNumber(speakers.annotatedItemCount) : '...'}
          muted={!speakers}
          compact
        />
        <KpiCard
          label={t('settings.dashboard.speaker_attributed_duration', { defaultValue: 'Speaker-Attributed Duration' })}
          value={speakers ? formatDuration(speakers.speakerAttributedDuration, t) : '...'}
          muted={!speakers}
          compact
        />
        <KpiCard
          label={t('settings.dashboard.identified_speakers', { defaultValue: 'Identified Speakers' })}
          value={speakers ? formatNumber(speakers.identifiedSpeakerCount) : '...'}
          muted={!speakers}
          compact
        />
        <KpiCard
          label={t('settings.dashboard.anonymous_slots', { defaultValue: 'Anonymous Speaker Slots' })}
          value={speakers ? formatNumber(speakers.anonymousSpeakerSlotCount) : '...'}
          muted={!speakers}
          compact
        />
      </div>

      <div className="settings-dashboard-speaker-detail-grid">
        <SpeakerOverviewCard
          speakers={speakers}
          segmentCoverage={segmentCoverage}
          durationCoverage={durationCoverage}
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
  return (
    <div className="settings-dashboard-panel">
      <div className="settings-dashboard-feature-grid">
        <KpiCard
          label={t('settings.dashboard.llm_call_count', { defaultValue: 'Successful Calls' })}
          value={formatNumber(llmUsage.totals.callCount)}
          badge={<Bot size={16} />}
          variant="feature"
          tone="info"
        />
        <KpiCard
          label={t('settings.dashboard.total_tokens', { defaultValue: 'Total Tokens' })}
          value={formatNumber(llmUsage.totals.totalTokens)}
          badge={<BarChart3 size={16} />}
          variant="feature"
          tone="accent"
        />
      </div>

      <div className="settings-dashboard-support-grid">
        <KpiCard
          label={t('settings.dashboard.calls_with_usage', { defaultValue: 'Calls With Usage' })}
          value={formatNumber(llmUsage.totals.callsWithUsage)}
        />
        <KpiCard
          label={t('settings.dashboard.calls_without_usage', { defaultValue: 'Calls Missing Usage' })}
          value={formatNumber(llmUsage.totals.callsWithoutUsage)}
          tone={llmUsage.totals.callsWithoutUsage > 0 ? 'warm' : 'default'}
        />
      </div>

      <div className="settings-dashboard-meta-rail">
        <div className="settings-dashboard-meta-pill">
          <BarChart3 size={14} />
          <span>
            {t('settings.dashboard.tokens_hint', {
              defaultValue: '{{prompt}} prompt / {{completion}} completion',
              prompt: formatNumber(llmUsage.totals.promptTokens),
              completion: formatNumber(llmUsage.totals.completionTokens),
            })}
          </span>
        </div>
        <div className="settings-dashboard-meta-pill">
          <Clock3 size={14} />
          <span>
            {llmUsage.startedAt
              ? t('settings.dashboard.tracking_since', {
                defaultValue: 'Tracking since {{date}}',
                date: new Date(llmUsage.startedAt).toLocaleString(),
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
                count: formatNumber(llmUsage.totals.callsWithoutUsage),
              })}
            </span>
          </div>
        )}
      </div>

      <div className="settings-dashboard-usage-grid">
        <TokenTrend points={llmUsage.recentDaily} t={t} />
        <UsageBreakdown
          title={t('settings.dashboard.by_provider', { defaultValue: 'By Provider' })}
          breakdown={llmUsage.byProvider}
          t={t}
        />
        <UsageBreakdown
          title={t('settings.dashboard.by_category', { defaultValue: 'By Category' })}
          breakdown={llmUsage.byCategory}
          t={t}
        />
      </div>
    </div>
  );
}
