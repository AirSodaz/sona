import type {
  HistoryWorkspaceQueryRequest,
  HistoryWorkspaceQueryResult,
  HistoryWorkspaceQueryScope,
} from '../../services/tauri/history';
import { ALL_ITEMS_SCOPE, INBOX_SCOPE } from './constants';

export type ProjectFilterType = 'all' | 'recording' | 'batch';
export type ProjectDateFilter = 'all' | 'today' | 'week' | 'month';
export type ProjectSortOrder = 'newest' | 'oldest' | 'duration_desc' | 'duration_asc' | 'title_asc';
export type ProjectBrowseScope = typeof ALL_ITEMS_SCOPE | typeof INBOX_SCOPE | string;

export type TranslationFn = (key: string, options?: Record<string, unknown>) => string;

export interface ProjectSummary {
  totalItems: number;
  totalDuration: number;
  latestTimestamp: number | null;
  recordingCount: number;
  batchCount: number;
}

export type WorkspaceQueryScope = HistoryWorkspaceQueryScope;
export type WorkspaceQueryRequest = HistoryWorkspaceQueryRequest;
export type WorkspaceQueryResult = HistoryWorkspaceQueryResult;

export interface ProjectSummaryChip {
  key: string;
  label: string;
  value: string;
  testId: string;
}

export interface RenameTarget {
  id: string;
  title: string;
  icon?: string;
  type?: 'recording' | 'batch';
}
