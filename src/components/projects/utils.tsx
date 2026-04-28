import React from 'react';
import type { HistoryItem as HistoryItemType } from '../../types/history';
import type { ProjectDefaults, ProjectRecord } from '../../types/project';
import { renderIcon } from '../IconPicker';
import { FolderIcon, InboxIcon, SummaryIcon } from '../Icons';
import { ALL_ITEMS_SCOPE, INBOX_SCOPE } from './constants';
import type {
  ProjectBrowseScope,
  ProjectDateFilter,
  ProjectSortOrder,
  TranslationFn,
} from './types';

function sortRuleSetIds(ids: string[]): string[] {
  return [...ids].sort();
}

export function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  return `${date.toLocaleDateString()} ${date.toLocaleTimeString()}`;
}

export function formatSummaryDuration(
  durationInSeconds: number,
  t: TranslationFn,
): string {
  const totalMinutes = Math.max(0, Math.round(durationInSeconds / 60));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours > 0) {
    return t('projects.summary_duration_hours', {
      hours,
      minutes,
      defaultValue: `${hours}h ${minutes}m`,
    });
  }

  return t('projects.summary_duration_minutes', {
    minutes: totalMinutes,
    defaultValue: `${totalMinutes}m`,
  });
}

export function matchesDateFilter(item: HistoryItemType, dateFilter: ProjectDateFilter): boolean {
  if (dateFilter === 'all') {
    return true;
  }

  const itemDate = new Date(item.timestamp);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  if (dateFilter === 'today') {
    return itemDate >= today;
  }

  if (dateFilter === 'week') {
    const weekAgo = new Date(today);
    weekAgo.setDate(weekAgo.getDate() - 7);
    return itemDate >= weekAgo;
  }

  const monthAgo = new Date(today);
  monthAgo.setMonth(monthAgo.getMonth() - 1);
  return itemDate >= monthAgo;
}

export function compareProjectItems(a: HistoryItemType, b: HistoryItemType, sortOrder: ProjectSortOrder): number {
  switch (sortOrder) {
    case 'oldest':
      return a.timestamp - b.timestamp;
    case 'duration_desc':
      return b.duration - a.duration || b.timestamp - a.timestamp;
    case 'duration_asc':
      return a.duration - b.duration || b.timestamp - a.timestamp;
    case 'title_asc':
      return a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }) || b.timestamp - a.timestamp;
    case 'newest':
    default:
      return b.timestamp - a.timestamp;
  }
}

export function buildComparableProjectSettingsSnapshot(input: {
  name: string;
  description: string;
  icon?: string;
  defaults: ProjectDefaults;
}) {
  return {
    name: input.name.trim(),
    description: input.description,
    icon: input.icon || '',
    summaryTemplateId: input.defaults.summaryTemplateId,
    translationLanguage: input.defaults.translationLanguage,
    polishPresetId: input.defaults.polishPresetId,
    exportFileNamePrefix: input.defaults.exportFileNamePrefix,
    enabledTextReplacementSetIds: sortRuleSetIds(input.defaults.enabledTextReplacementSetIds),
    enabledHotwordSetIds: sortRuleSetIds(input.defaults.enabledHotwordSetIds),
    enabledPolishKeywordSetIds: sortRuleSetIds(input.defaults.enabledPolishKeywordSetIds),
    enabledSpeakerProfileIds: sortRuleSetIds(input.defaults.enabledSpeakerProfileIds),
  };
}

export function renderScopeIcon(scope: ProjectBrowseScope, project?: ProjectRecord | null): React.ReactNode {
  if (scope === ALL_ITEMS_SCOPE) {
    return <SummaryIcon />;
  }

  if (scope === INBOX_SCOPE) {
    return <InboxIcon />;
  }

  return renderIcon(project?.icon, <FolderIcon />);
}

interface RailItemContentProps {
  icon: React.ReactNode;
  title: string;
  description?: string;
}

export function RailItemContent({ icon, title, description }: RailItemContentProps): React.JSX.Element {
  return (
    <div className="projects-rail-item-main">
      <span className="projects-rail-item-icon" aria-hidden="true">
        {icon}
      </span>
      <div className="projects-rail-item-copy">
        <strong>{title}</strong>
        {description ? <span>{description}</span> : null}
      </div>
    </div>
  );
}
