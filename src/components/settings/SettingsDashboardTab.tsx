/* eslint-disable react-refresh/only-export-components */

import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  AlertCircle,
  BarChart3,
  Bot,
  FileText,
  LoaderCircle,
} from 'lucide-react';
import type { DashboardSnapshot } from '../../types/dashboard';
import { dashboardService } from '../../services/dashboardService';
import { normalizeError } from '../../utils/errorUtils';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import {
  ContentOverviewSection,
  LlmUsagePanel,
  SpeakerInsightsSection,
} from './dashboard/sections';
import './SettingsDashboardTab.css';

export {
  buildTrendGeometry,
  normalizeTrendPoints,
} from './dashboard/trendGeometry';

export function SettingsDashboardTab(): React.JSX.Element {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [isFastLoading, setIsFastLoading] = useState(true);
  const [isDeepLoading, setIsDeepLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    let cancelled = false;

    async function loadDashboard(): Promise<void> {
      setIsFastLoading(true);
      setIsDeepLoading(false);
      setError(null);
      setSnapshot(null);

      try {
        const fastSnapshot = await dashboardService.getFastSnapshot();
        if (cancelled) {
          return;
        }

        setSnapshot(fastSnapshot);
        setIsFastLoading(false);

        if (fastSnapshot.content.overview.itemCount === 0) {
          return;
        }

        setIsDeepLoading(true);

        try {
          const deepSnapshot = await dashboardService.getDeepSnapshot();
          if (!cancelled) {
            setSnapshot(deepSnapshot);
          }
        } catch (deepError) {
          if (!cancelled) {
            setError(normalizeError(deepError).message);
          }
        } finally {
          if (!cancelled) {
            setIsDeepLoading(false);
          }
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(normalizeError(loadError).message);
          setIsFastLoading(false);
        }
      }
    }

    void loadDashboard();

    return () => {
      cancelled = true;
    };
  }, [reloadToken]);

  if (isFastLoading && !snapshot) {
    return (
      <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
        <div className="settings-dashboard-state loading" data-testid="dashboard-loading">
          <LoaderCircle className="spin" size={18} />
          <span>{t('settings.dashboard.loading', { defaultValue: 'Loading dashboard...' })}</span>
        </div>
      </SettingsTabContainer>
    );
  }

  if (!snapshot) {
    return (
      <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
        <div className="settings-dashboard-state error" data-testid="dashboard-error">
          <AlertCircle size={18} />
          <div>
            <div className="settings-dashboard-state-title">{t('settings.dashboard.error_title', { defaultValue: 'Dashboard unavailable' })}</div>
            <div className="settings-dashboard-note">{error || t('settings.dashboard.error_body', { defaultValue: 'Try reloading dashboard data.' })}</div>
          </div>
          <button className="btn" onClick={() => setReloadToken((value) => value + 1)}>
            {t('settings.dashboard.retry', { defaultValue: 'Retry' })}
          </button>
        </div>
      </SettingsTabContainer>
    );
  }

  const { overview, speakers } = snapshot.content;
  const llmUsage = snapshot.llmUsage;
  const isEmpty = overview.itemCount === 0 && llmUsage.totals.callCount === 0;

  if (isEmpty) {
    return (
      <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
        <SettingsPageHeader
          icon={<BarChart3 size={28} />}
          title={t('settings.dashboard.title', { defaultValue: 'Dashboard' })}
          description={t('settings.dashboard.description', {
            defaultValue: 'Review global content and LLM usage without leaving Settings.',
          })}
        />
        <div className="settings-dashboard-empty-page" data-testid="dashboard-empty">
          <div className="settings-dashboard-state-title">{t('settings.dashboard.empty_title', { defaultValue: 'No dashboard data yet' })}</div>
          <div className="settings-dashboard-note">
            {t('settings.dashboard.empty_body', {
              defaultValue: 'Saved transcripts and successful LLM calls will start appearing here automatically.',
            })}
          </div>
        </div>
      </SettingsTabContainer>
    );
  }

  return (
    <SettingsTabContainer id="settings-panel-dashboard" ariaLabelledby="settings-tab-dashboard">
      <SettingsPageHeader
        icon={<BarChart3 size={28} />}
        title={t('settings.dashboard.title', { defaultValue: 'Dashboard' })}
        description={t('settings.dashboard.description', {
          defaultValue: 'Review global content and LLM usage without leaving Settings.',
        })}
      />

      <SettingsSection
        title={t('settings.dashboard.global_content', { defaultValue: 'Global Content' })}
        description={t('settings.dashboard.global_content_description', {
          defaultValue: 'Saved content volume, where it lives, and how much transcript data already has speaker attribution.',
        })}
        icon={<FileText size={20} />}
      >
        <div className="settings-dashboard-panel">
          <ContentOverviewSection
            overview={overview}
            isDeepLoading={isDeepLoading}
            error={error}
            t={t}
          />
          <SpeakerInsightsSection
            speakers={speakers}
            isDeepLoading={isDeepLoading}
            error={error}
            t={t}
          />
        </div>
      </SettingsSection>

      <SettingsSection
        title={t('settings.dashboard.llm_usage', { defaultValue: 'LLM Usage' })}
        description={t('settings.dashboard.llm_usage_description', {
          defaultValue: 'All-time successful LLM calls tracked since this analytics file started, including connection tests.',
        })}
        icon={<Bot size={20} />}
      >
        <LlmUsagePanel llmUsage={llmUsage} t={t} />
      </SettingsSection>
    </SettingsTabContainer>
  );
}
