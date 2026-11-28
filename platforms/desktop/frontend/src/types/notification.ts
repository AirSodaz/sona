import type React from 'react';

export type NotificationSource = 'task' | 'update' | 'onboarding' | 'download';

/**
 * Priority determines the section a notification belongs to.
 *  - action:  needs user intervention (failed tasks, available updates, onboarding)
 *  - active:  in-progress work (running tasks, downloading updates/models)
 *  - info:    completed or informational (succeeded, cancelled)
 */
export type NotificationPriority = 'action' | 'active' | 'info';

/**
 * Semantic tone mapped to CSS accent colors. Replaces the previous 8-value
 * `NotificationTone` with 5 semantic values that map cleanly to the design system.
 */
export type NotificationTone = 'info' | 'success' | 'warning' | 'error' | 'accent';

export type NotificationActionVariant = 'primary' | 'secondary' | 'soft';

export interface NotificationAction {
  id: string;
  label: string;
  variant: NotificationActionVariant;
  disabled?: boolean;
  run: () => void | Promise<void>;
}

export interface NotificationSubItem {
  id: string;
  title: string;
  subtitle?: string | null;
  tone: NotificationTone;
  progress?: number;
  detail?: string | null;
  actions?: NotificationAction[];
}

export interface NotificationEntry {
  id: string;
  source: NotificationSource;
  priority: NotificationPriority;
  tone: NotificationTone;
  icon: React.ReactNode;
  title: string;
  body?: string | null;
  bodyClassName?: string;
  /** Millisecond timestamp for sort ordering. */
  timestamp: number;
  /** Explicit pinned rank (higher ranks appear above regular timestamped entries). */
  pinnedRank?: number;
  /** Formatted relative or clock timestamp label displayed subtly in the card. */
  timestampLabel?: string | null;
  /** 0–100 progress value for tasks/downloads with a progress bar. */
  progress?: number;
  /** Primary row actions (retry, install, resume, etc.). */
  actions: NotificationAction[];
  /** Dismiss / close action rendered as an icon button. */
  closeAction?: NotificationAction;
  /** Extra detail text rendered below the main content (stage label, error). */
  detail?: string | null;
  /** Collapsible sub-items for aggregated notifications (e.g. batch transcription). */
  items?: NotificationSubItem[];
  /** Custom label for the sub-items expand toggle. */
  itemsLabel?: string;
  /** Additional support ReactNode (progress bar, status badge). */
  support?: React.ReactNode;
  /** Optional CSS class on the card `<li>`. */
  itemClassName?: string;
  /** When set, the main content area is clickable. */
  onOpen?: () => void;
  /** Optional initial expanded state for collapsible content. */
  defaultExpanded?: boolean;
}

export interface NotificationGroup {
  action: NotificationEntry[];
  active: NotificationEntry[];
  info: NotificationEntry[];
}
