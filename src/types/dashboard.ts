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
  dateLabel: string;
  itemCount: number;
  itemCountDisplay: string;
  durationSeconds: number;
  durationDisplay: string;
}

export interface DashboardOverviewStats {
  itemCount: number;
  itemCountDisplay: string;
  projectCount: number;
  projectCountDisplay: string;
  totalDurationSeconds: number;
  totalDurationDisplay: string;
  transcriptCharacterCount?: number;
  transcriptCharacterCountDisplay?: string;
  recordingCount: number;
  recordingCountDisplay: string;
  batchCount: number;
  batchCountDisplay: string;
  inboxCount: number;
  inboxCountDisplay: string;
  projectAssignedCount: number;
  projectAssignedCountDisplay: string;
  recentDailyItems: DashboardContentTrendPoint[];
  isDeepLoaded: boolean;
}

export interface DashboardSpeakerLeader {
  speakerId: string;
  label: string;
  durationSeconds: number;
  durationDisplay: string;
  segmentCount: number;
  segmentCountDisplay: string;
  itemCount: number;
  itemCountDisplay: string;
}

export interface DashboardSpeakerStats {
  annotatedItemCount: number;
  annotatedItemCountDisplay: string;
  speakerAttributedDuration: number;
  speakerAttributedDurationDisplay: string;
  identifiedSpeakerCount: number;
  identifiedSpeakerCountDisplay: string;
  anonymousSpeakerSlotCount: number;
  anonymousSpeakerSlotCountDisplay: string;
  speakerTaggedSegmentCount: number;
  speakerTaggedSegmentCountDisplay: string;
  totalSegmentCount: number;
  totalSegmentCountDisplay: string;
  totalSegmentDuration: number;
  totalSegmentDurationDisplay: string;
  identifiedDuration: number;
  identifiedDurationDisplay: string;
  anonymousDuration: number;
  anonymousDurationDisplay: string;
  segmentCoverageRatio: number;
  segmentCoverageLabel: string;
  durationCoverageRatio: number;
  durationCoverageLabel: string;
  topIdentifiedSpeakers: DashboardSpeakerLeader[];
  topIdentifiedSpeakerRows: DashboardSpeakerLeader[];
  topIdentifiedSpeakerMaxValue: number;
  isDeepLoaded: boolean;
}

export interface DashboardContentStats {
  overview: DashboardOverviewStats;
  speakers: DashboardSpeakerStats | null;
}

export interface LlmUsageRawAggregateBucket {
  callCount: number;
  callsWithUsage: number;
  callsWithoutUsage: number;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

export interface LlmUsageAggregateBucket extends LlmUsageRawAggregateBucket {
  callCountDisplay: string;
  callsWithUsageDisplay: string;
  callsWithoutUsageDisplay: string;
  promptTokensDisplay: string;
  completionTokensDisplay: string;
  totalTokensDisplay: string;
}

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

export interface DashboardLlmUsageBreakdown<TValue extends string> {
  key: TValue;
  label: string;
  stats: LlmUsageAggregateBucket;
  value: number;
  valueDisplay: string;
}

export interface DashboardLlmUsageTrendPoint extends LlmUsageAggregateBucket {
  date: string;
  dateLabel: string;
}

export interface DashboardLlmUsageStats {
  startedAt?: string;
  lastUpdatedAt?: string;
  trackingSinceDisplay?: string;
  lastUpdatedDisplay?: string;
  totals: LlmUsageAggregateBucket;
  byProvider: DashboardLlmUsageBreakdown<LlmProvider>[];
  byProviderTopRows: DashboardLlmUsageBreakdown<LlmProvider>[];
  byProviderMaxValue: number;
  byCategory: DashboardLlmUsageBreakdown<LlmUsageCategory>[];
  byCategoryTopRows: DashboardLlmUsageBreakdown<LlmUsageCategory>[];
  byCategoryMaxValue: number;
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
