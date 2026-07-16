import type {
  ContentStats_Serialize as GeneratedDashboardContentStats,
  ContentTrendPoint as GeneratedDashboardContentTrendPoint,
  DashboardSnapshotDomainModel_Serialize as GeneratedDashboardSnapshot,
  DashboardUsageBucket as GeneratedDashboardUsageBucket,
  LlmUsageDashboardStats_Serialize as GeneratedDashboardLlmUsageStats,
  OverviewStats_Serialize as GeneratedDashboardOverviewStats,
  SpeakerLeader as GeneratedDashboardSpeakerLeader,
  SpeakerStats as GeneratedDashboardSpeakerStats,
  UsageBreakdown as GeneratedDashboardLlmUsageBreakdown,
  UsageTrendPoint as GeneratedDashboardLlmUsageTrendPoint,
} from '../bindings';
import type { LlmConfig, LlmProvider } from './transcript';

export type LlmUsageCategory =
  | 'summary'
  | 'translation'
  | 'polish'
  | 'title_generation'
  | 'connection_test'
  | 'generic';

export type LlmGenerateUsageSource =
  | 'title_generation'
  | 'connection_test'
  | 'generic';

export interface LlmGenerateCommandRequest {
  config: LlmConfig;
  input: string;
  source?: LlmGenerateUsageSource;
}

export type DashboardContentTrendPoint = GeneratedDashboardContentTrendPoint;
export type DashboardOverviewStats = GeneratedDashboardOverviewStats;
export type DashboardSpeakerLeader = GeneratedDashboardSpeakerLeader;
export type DashboardSpeakerStats = GeneratedDashboardSpeakerStats;
export type DashboardContentStats = GeneratedDashboardContentStats;

export interface LlmUsageRawAggregateBucket {
  callCount: number;
  callsWithUsage: number;
  callsWithoutUsage: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

export type LlmUsageAggregateBucket = GeneratedDashboardUsageBucket;

export type LlmUsageByProvider = Partial<Record<LlmProvider, LlmUsageRawAggregateBucket>>;
export type LlmUsageByCategory = Partial<Record<LlmUsageCategory, LlmUsageRawAggregateBucket>>;

export interface LlmUsageStatsFile {
  schemaVersion: number;
  startedAt?: string;
  lastUpdatedAt?: string;
  totals: LlmUsageRawAggregateBucket;
  byProvider: LlmUsageByProvider;
  byCategory: LlmUsageByCategory;
  daily: Record<string, LlmUsageRawAggregateBucket>;
}

export type DashboardLlmUsageBreakdown<TValue extends string = string> = Omit<
  GeneratedDashboardLlmUsageBreakdown,
  'key'
> & { key: TValue };

export type DashboardLlmUsageTrendPoint = GeneratedDashboardLlmUsageTrendPoint;
export type DashboardLlmUsageStats = GeneratedDashboardLlmUsageStats;
export type DashboardSnapshot = GeneratedDashboardSnapshot;

export interface LlmUsageEventPayload {
  occurredAt: string;
  provider: LlmProvider;
  model: string;
  category: LlmUsageCategory;
  usage?: {
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
  } | null;
}
