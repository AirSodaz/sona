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

export interface DashboardContentTrendPoint {
  date: string;
  itemCount: number;
  durationSeconds: number;
}

export interface DashboardOverviewStats {
  itemCount: number;
  projectCount: number;
  totalDurationSeconds: number;
  transcriptCharacterCount?: number;
  recordingCount: number;
  batchCount: number;
  inboxCount: number;
  projectAssignedCount: number;
  recentDailyItems: DashboardContentTrendPoint[];
  isDeepLoaded: boolean;
}

export interface DashboardSpeakerLeader {
  speakerId: string;
  label: string;
  durationSeconds: number;
  segmentCount: number;
  itemCount: number;
}

export interface DashboardSpeakerStats {
  annotatedItemCount: number;
  speakerAttributedDuration: number;
  identifiedSpeakerCount: number;
  anonymousSpeakerSlotCount: number;
  speakerTaggedSegmentCount: number;
  totalSegmentCount: number;
  totalSegmentDuration: number;
  identifiedDuration: number;
  anonymousDuration: number;
  topIdentifiedSpeakers: DashboardSpeakerLeader[];
  isDeepLoaded: boolean;
}

export interface DashboardContentStats {
  overview: DashboardOverviewStats;
  speakers: DashboardSpeakerStats | null;
}

export interface LlmUsageAggregateBucket {
  callCount: number;
  callsWithUsage: number;
  callsWithoutUsage: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

export type LlmUsageByProvider = Partial<Record<LlmProvider, LlmUsageAggregateBucket>>;
export type LlmUsageByCategory = Partial<Record<LlmUsageCategory, LlmUsageAggregateBucket>>;

export interface LlmUsageStatsFile {
  schemaVersion: number;
  startedAt?: string;
  lastUpdatedAt?: string;
  totals: LlmUsageAggregateBucket;
  byProvider: LlmUsageByProvider;
  byCategory: LlmUsageByCategory;
  daily: Record<string, LlmUsageAggregateBucket>;
}

export interface DashboardLlmUsageBreakdown<TValue extends string> {
  key: TValue;
  stats: LlmUsageAggregateBucket;
}

export interface DashboardLlmUsageTrendPoint extends LlmUsageAggregateBucket {
  date: string;
}

export interface DashboardLlmUsageStats {
  startedAt?: string;
  lastUpdatedAt?: string;
  totals: LlmUsageAggregateBucket;
  byProvider: DashboardLlmUsageBreakdown<LlmProvider>[];
  byCategory: DashboardLlmUsageBreakdown<LlmUsageCategory>[];
  recentDaily: DashboardLlmUsageTrendPoint[];
}

export interface DashboardSnapshot {
  content: DashboardContentStats;
  llmUsage: DashboardLlmUsageStats;
  generatedAt: string;
}

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
