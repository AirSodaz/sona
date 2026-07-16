/* eslint-disable react-refresh/only-export-components */

import React from 'react';
import type { ProjectDefaults, ProjectRecord } from '../../types/project';
import { renderIcon } from '../IconPicker';
import { FolderIcon, InboxIcon, SummaryIcon, TrashIcon } from '../Icons';
import { ALL_ITEMS_SCOPE, TRASH_SCOPE, UNTAGGED_SCOPE } from './constants';
import type {
  ProjectBrowseScope,
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

export function buildComparableProjectSettingsSnapshot(input: {
  name: string;
  description: string;
  icon?: string;
  color?: string;
  defaults: ProjectDefaults;
}) {
  return {
    name: input.name.trim(),
    description: input.description,
    icon: input.icon || '',
    color: input.color || '#64748b',
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

  if (scope === UNTAGGED_SCOPE) {
    return <InboxIcon />;
  }

  if (scope === TRASH_SCOPE) {
    return <TrashIcon />;
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
