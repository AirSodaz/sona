import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { BaseDirectory, exists, mkdir, readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import type {
  DashboardLlmUsageBreakdown,
  DashboardLlmUsageStats,
  DashboardLlmUsageTrendPoint,
  LlmUsageAggregateBucket,
  LlmUsageCategory,
  LlmUsageEventPayload,
  LlmUsageStatsFile,
} from '../types/dashboard';
import type { LlmProvider } from '../types/transcript';
import { logger } from '../utils/logger';
import { TauriEvent } from './tauri/events';

const ANALYTICS_DIR = 'analytics';
const LLM_USAGE_FILE = `${ANALYTICS_DIR}/llm-usage.json`;
const LLM_USAGE_RECORDED_EVENT = TauriEvent.llm.usageRecorded;
const LLM_USAGE_SCHEMA_VERSION = 1;
const RECENT_DAILY_WINDOW = 30;

function createEmptyUsageBucket(): LlmUsageAggregateBucket {
  return {
    callCount: 0,
    callsWithUsage: 0,
    callsWithoutUsage: 0,
    promptTokens: 0,
    completionTokens: 0,
    totalTokens: 0,
  };
}

function normalizeCount(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.round(value)
    : 0;
}

function normalizeUsageBucket(value: unknown): LlmUsageAggregateBucket {
  const source = value && typeof value === 'object'
    ? value as Partial<LlmUsageAggregateBucket>
    : {};

  return {
    callCount: normalizeCount(source.callCount),
    callsWithUsage: normalizeCount(source.callsWithUsage),
    callsWithoutUsage: normalizeCount(source.callsWithoutUsage),
    promptTokens: normalizeCount(source.promptTokens),
    completionTokens: normalizeCount(source.completionTokens),
    totalTokens: normalizeCount(source.totalTokens),
  };
}

function createEmptyStatsFile(): LlmUsageStatsFile {
  return {
    schemaVersion: LLM_USAGE_SCHEMA_VERSION,
    totals: createEmptyUsageBucket(),
    byProvider: {},
    byCategory: {},
    daily: {},
  };
}

function normalizeStatsFile(raw: unknown): LlmUsageStatsFile {
  const source = raw && typeof raw === 'object'
    ? raw as Partial<LlmUsageStatsFile>
    : {};

  const byProviderSource = source.byProvider && typeof source.byProvider === 'object'
    ? source.byProvider
    : {};
  const byCategorySource = source.byCategory && typeof source.byCategory === 'object'
    ? source.byCategory
    : {};
  const dailySource = source.daily && typeof source.daily === 'object'
    ? source.daily
    : {};

  const byProvider: LlmUsageStatsFile['byProvider'] = {};
  const byCategory: LlmUsageStatsFile['byCategory'] = {};
  const daily: LlmUsageStatsFile['daily'] = {};

  Object.entries(byProviderSource).forEach(([key, value]) => {
    byProvider[key as LlmProvider] = normalizeUsageBucket(value);
  });

  Object.entries(byCategorySource).forEach(([key, value]) => {
    byCategory[key as LlmUsageCategory] = normalizeUsageBucket(value);
  });

  Object.entries(dailySource).forEach(([key, value]) => {
    daily[key] = normalizeUsageBucket(value);
  });

  return {
    schemaVersion: normalizeCount(source.schemaVersion) || LLM_USAGE_SCHEMA_VERSION,
    startedAt: typeof source.startedAt === 'string' && source.startedAt.trim() ? source.startedAt : undefined,
    lastUpdatedAt: typeof source.lastUpdatedAt === 'string' && source.lastUpdatedAt.trim() ? source.lastUpdatedAt : undefined,
    totals: normalizeUsageBucket(source.totals),
    byProvider,
    byCategory,
    daily,
  };
}

function getLocalDateKey(input: string | Date): string {
  const date = input instanceof Date ? input : new Date(input);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function addUsageBucket(target: LlmUsageAggregateBucket, usage: LlmUsageAggregateBucket): void {
  target.callCount += usage.callCount;
  target.callsWithUsage += usage.callsWithUsage;
  target.callsWithoutUsage += usage.callsWithoutUsage;
  target.promptTokens += usage.promptTokens;
  target.completionTokens += usage.completionTokens;
  target.totalTokens += usage.totalTokens;
}

function sanitizeEventUsage(usage: LlmUsageEventPayload['usage']): LlmUsageAggregateBucket | null {
  if (!usage) {
    return null;
  }

  const promptTokens = normalizeCount(usage.promptTokens);
  const completionTokens = normalizeCount(usage.completionTokens);
  const totalTokens = normalizeCount(usage.totalTokens) || promptTokens + completionTokens;

  if (promptTokens === 0 && completionTokens === 0 && totalTokens === 0) {
    return null;
  }

  return {
    callCount: 1,
    callsWithUsage: 1,
    callsWithoutUsage: 0,
    promptTokens,
    completionTokens,
    totalTokens,
  };
}

function createCallOnlyBucket(): LlmUsageAggregateBucket {
  return {
    callCount: 1,
    callsWithUsage: 0,
    callsWithoutUsage: 1,
    promptTokens: 0,
    completionTokens: 0,
    totalTokens: 0,
  };
}

function upsertBucket<T extends string>(
  collection: Partial<Record<T, LlmUsageAggregateBucket>>,
  key: T,
): LlmUsageAggregateBucket {
  if (!collection[key]) {
    collection[key] = createEmptyUsageBucket();
  }

  return collection[key]!;
}

function buildRecentDailyTrend(daily: Record<string, LlmUsageAggregateBucket>): DashboardLlmUsageTrendPoint[] {
  const points: DashboardLlmUsageTrendPoint[] = [];
  const today = new Date();

  for (let offset = RECENT_DAILY_WINDOW - 1; offset >= 0; offset -= 1) {
    const date = new Date(today);
    date.setDate(today.getDate() - offset);
    const key = getLocalDateKey(date);
    points.push({
      date: key,
      ...normalizeUsageBucket(daily[key]),
    });
  }

  return points;
}

function toSortedBreakdown<TValue extends string>(
  collection: Partial<Record<TValue, LlmUsageAggregateBucket>>,
): DashboardLlmUsageBreakdown<TValue>[] {
  return Object.entries(collection)
    .map(([key, stats]) => ({
      key: key as TValue,
      stats: normalizeUsageBucket(stats),
    }))
    .filter(({ stats }) => stats.callCount > 0)
    .sort((left, right) => (
      right.stats.totalTokens - left.stats.totalTokens
      || right.stats.callCount - left.stats.callCount
      || left.key.localeCompare(right.key)
    ));
}

function toDashboardStats(statsFile: LlmUsageStatsFile): DashboardLlmUsageStats {
  return {
    startedAt: statsFile.startedAt,
    lastUpdatedAt: statsFile.lastUpdatedAt,
    totals: normalizeUsageBucket(statsFile.totals),
    byProvider: toSortedBreakdown(statsFile.byProvider),
    byCategory: toSortedBreakdown(statsFile.byCategory),
    recentDaily: buildRecentDailyTrend(statsFile.daily),
  };
}

class LlmUsageService {
  private initPromise: Promise<void> | null = null;
  private unlisten: UnlistenFn | null = null;
  private writeQueue: Promise<void> = Promise.resolve();

  async init(): Promise<void> {
    if (!this.initPromise) {
      this.initPromise = this.initialize();
    }

    return this.initPromise;
  }

  async getStats(): Promise<DashboardLlmUsageStats> {
    const statsFile = await this.readStatsFile();
    return toDashboardStats(statsFile);
  }

  async recordUsage(payload: LlmUsageEventPayload): Promise<void> {
    const normalizedOccurredAt = typeof payload.occurredAt === 'string' && payload.occurredAt.trim()
      ? payload.occurredAt
      : new Date().toISOString();
    const usageBucket = sanitizeEventUsage(payload.usage) ?? createCallOnlyBucket();

    const stats = await this.readStatsFile();
    stats.startedAt = stats.startedAt || normalizedOccurredAt;
    stats.lastUpdatedAt = normalizedOccurredAt;

    addUsageBucket(stats.totals, usageBucket);
    addUsageBucket(upsertBucket(stats.byProvider, payload.provider), usageBucket);
    addUsageBucket(upsertBucket(stats.byCategory, payload.category), usageBucket);

    const dateKey = getLocalDateKey(normalizedOccurredAt);
    addUsageBucket(upsertBucket(stats.daily, dateKey), usageBucket);

    await this.writeStatsFile(stats);
  }

  private async initialize(): Promise<void> {
    await this.ensureStorage();

    if (this.unlisten) {
      return;
    }

    this.unlisten = await listen<LlmUsageEventPayload>(LLM_USAGE_RECORDED_EVENT, ({ payload }) => {
      this.writeQueue = this.writeQueue
        .then(() => this.recordUsage(payload))
        .catch((error) => {
          logger.error('[LLM Usage] Failed to persist usage event:', error);
        });
    });
  }

  private async ensureStorage(): Promise<void> {
    const analyticsExists = await exists(ANALYTICS_DIR, { baseDir: BaseDirectory.AppLocalData });
    if (!analyticsExists) {
      await mkdir(ANALYTICS_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
    }

    const usageFileExists = await exists(LLM_USAGE_FILE, { baseDir: BaseDirectory.AppLocalData });
    if (!usageFileExists) {
      await writeTextFile(
        LLM_USAGE_FILE,
        JSON.stringify(createEmptyStatsFile(), null, 2),
        { baseDir: BaseDirectory.AppLocalData },
      );
    }
  }

  private async readStatsFile(): Promise<LlmUsageStatsFile> {
    try {
      await this.ensureStorage();
      const content = await readTextFile(LLM_USAGE_FILE, { baseDir: BaseDirectory.AppLocalData });
      return normalizeStatsFile(JSON.parse(content));
    } catch (error) {
      logger.error('[LLM Usage] Failed to read usage file, falling back to empty state:', error);
      return createEmptyStatsFile();
    }
  }

  private async writeStatsFile(stats: LlmUsageStatsFile): Promise<void> {
    await this.ensureStorage();
    await writeTextFile(
      LLM_USAGE_FILE,
      JSON.stringify(stats, null, 2),
      { baseDir: BaseDirectory.AppLocalData },
    );
  }
}

export const llmUsageService = new LlmUsageService();
