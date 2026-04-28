import { BaseDirectory, readTextFile } from '@tauri-apps/plugin-fs';
import type {
  DashboardContentTrendPoint,
  DashboardOverviewStats,
  DashboardSnapshot,
  DashboardSpeakerLeader,
  DashboardSpeakerStats,
} from '../types/dashboard';
import type { HistoryItem } from '../types/history';
import { normalizeSpeakerTag } from '../types/speaker';
import { llmUsageService } from './llmUsageService';
import { logger } from '../utils/logger';

const HISTORY_INDEX_PATH = 'history/index.json';
const PROJECT_INDEX_PATH = 'projects/index.json';
const RECENT_DAILY_WINDOW = 30;

interface ParsedTranscriptSegment {
  text: string;
  durationSeconds: number;
  speaker?: ReturnType<typeof normalizeSpeakerTag>;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function toNonNegativeNumber(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, value)
    : 0;
}

function getLocalDateKey(input: string | number | Date): string {
  const date = input instanceof Date ? input : new Date(input);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function normalizeHistoryItem(input: unknown): HistoryItem | null {
  if (!isRecord(input) || !isNonEmptyString(input.id)) {
    return null;
  }

  return {
    id: input.id.trim(),
    timestamp: toNonNegativeNumber(input.timestamp),
    duration: toNonNegativeNumber(input.duration),
    audioPath: isNonEmptyString(input.audioPath) ? input.audioPath.trim() : '',
    transcriptPath: isNonEmptyString(input.transcriptPath) ? input.transcriptPath.trim() : '',
    title: isNonEmptyString(input.title) ? input.title.trim() : '',
    previewText: isNonEmptyString(input.previewText) ? input.previewText : '',
    icon: isNonEmptyString(input.icon) ? input.icon : undefined,
    type: input.type === 'batch' ? 'batch' : 'recording',
    searchContent: isNonEmptyString(input.searchContent) ? input.searchContent : '',
    projectId: isNonEmptyString(input.projectId) ? input.projectId.trim() : null,
  };
}

function createRecentDailyTrend(historyItems: HistoryItem[]): DashboardContentTrendPoint[] {
  const aggregates = new Map<string, DashboardContentTrendPoint>();

  historyItems.forEach((item) => {
    const key = getLocalDateKey(item.timestamp);
    const existing = aggregates.get(key);
    if (existing) {
      existing.itemCount += 1;
      existing.durationSeconds += item.duration;
      return;
    }

    aggregates.set(key, {
      date: key,
      itemCount: 1,
      durationSeconds: item.duration,
    });
  });

  const today = new Date();
  const trend: DashboardContentTrendPoint[] = [];

  for (let offset = RECENT_DAILY_WINDOW - 1; offset >= 0; offset -= 1) {
    const date = new Date(today);
    date.setDate(today.getDate() - offset);
    const key = getLocalDateKey(date);
    trend.push(aggregates.get(key) || {
      date: key,
      itemCount: 0,
      durationSeconds: 0,
    });
  }

  return trend;
}

function createOverview(historyItems: HistoryItem[], projectCount: number, isDeepLoaded: boolean): DashboardOverviewStats {
  const recordingCount = historyItems.filter((item) => item.type !== 'batch').length;
  const batchCount = historyItems.length - recordingCount;
  const inboxCount = historyItems.filter((item) => !item.projectId).length;

  return {
    itemCount: historyItems.length,
    projectCount,
    totalDurationSeconds: historyItems.reduce((sum, item) => sum + item.duration, 0),
    transcriptCharacterCount: undefined,
    recordingCount,
    batchCount,
    inboxCount,
    projectAssignedCount: historyItems.length - inboxCount,
    recentDailyItems: createRecentDailyTrend(historyItems),
    isDeepLoaded,
  };
}

function createEmptySpeakerStats(isDeepLoaded: boolean): DashboardSpeakerStats {
  return {
    annotatedItemCount: 0,
    speakerAttributedDuration: 0,
    identifiedSpeakerCount: 0,
    anonymousSpeakerSlotCount: 0,
    speakerTaggedSegmentCount: 0,
    totalSegmentCount: 0,
    totalSegmentDuration: 0,
    identifiedDuration: 0,
    anonymousDuration: 0,
    topIdentifiedSpeakers: [],
    isDeepLoaded,
  };
}

function parseTranscriptSegments(input: unknown): ParsedTranscriptSegment[] {
  if (!Array.isArray(input)) {
    return [];
  }

  return input.flatMap((segment) => {
    if (!isRecord(segment)) {
      return [];
    }

    const text = typeof segment.text === 'string' ? segment.text : '';
    const start = toNonNegativeNumber(segment.start);
    const end = toNonNegativeNumber(segment.end);

    return [{
      text,
      durationSeconds: Math.max(0, end - start),
      speaker: normalizeSpeakerTag(segment.speaker),
    }];
  });
}

async function readIndexArray(path: string): Promise<unknown[]> {
  const content = await readTextFile(path, { baseDir: BaseDirectory.AppLocalData });
  const parsed = JSON.parse(content);
  if (!Array.isArray(parsed)) {
    throw new Error(`${path} is not a JSON array`);
  }

  return parsed;
}

async function readHistoryItems(): Promise<HistoryItem[]> {
  const rawItems = await readIndexArray(HISTORY_INDEX_PATH);
  return rawItems
    .map((item) => normalizeHistoryItem(item))
    .filter((item): item is HistoryItem => !!item);
}

async function readProjectCount(): Promise<number> {
  const rawProjects = await readIndexArray(PROJECT_INDEX_PATH);
  return rawProjects.length;
}

async function aggregateTranscriptAnalytics(historyItems: HistoryItem[]): Promise<{
  transcriptCharacterCount: number;
  speakers: DashboardSpeakerStats;
}> {
  const speakers = createEmptySpeakerStats(true);
  let transcriptCharacterCount = 0;
  const identifiedSpeakerIds = new Set<string>();
  const leaderMap = new Map<string, DashboardSpeakerLeader & { itemIds: Set<string> }>();

  for (const item of historyItems) {
    if (!item.transcriptPath) {
      continue;
    }

    try {
      const content = await readTextFile(`history/${item.transcriptPath}`, {
        baseDir: BaseDirectory.AppLocalData,
      });
      const segments = parseTranscriptSegments(JSON.parse(content));

      let itemHasSpeaker = false;
      const anonymousIdsInItem = new Set<string>();

      segments.forEach((segment) => {
        transcriptCharacterCount += segment.text.length;
        speakers.totalSegmentCount += 1;
        speakers.totalSegmentDuration += segment.durationSeconds;

        if (!segment.speaker) {
          return;
        }

        itemHasSpeaker = true;
        speakers.speakerTaggedSegmentCount += 1;
        speakers.speakerAttributedDuration += segment.durationSeconds;

        if (segment.speaker.kind === 'identified') {
          speakers.identifiedDuration += segment.durationSeconds;
          identifiedSpeakerIds.add(segment.speaker.id);

          const existing = leaderMap.get(segment.speaker.id);
          if (existing) {
            existing.durationSeconds += segment.durationSeconds;
            existing.segmentCount += 1;
            existing.itemIds.add(item.id);
            return;
          }

          leaderMap.set(segment.speaker.id, {
            speakerId: segment.speaker.id,
            label: segment.speaker.label,
            durationSeconds: segment.durationSeconds,
            segmentCount: 1,
            itemCount: 1,
            itemIds: new Set([item.id]),
          });
          return;
        }

        speakers.anonymousDuration += segment.durationSeconds;
        anonymousIdsInItem.add(segment.speaker.id);
      });

      if (itemHasSpeaker) {
        speakers.annotatedItemCount += 1;
      }

      speakers.anonymousSpeakerSlotCount += anonymousIdsInItem.size;
    } catch (error) {
      logger.warn('[Dashboard] Skipping transcript during deep scan:', item.transcriptPath, error);
      continue;
    }
  }

  speakers.identifiedSpeakerCount = identifiedSpeakerIds.size;
  speakers.topIdentifiedSpeakers = [...leaderMap.values()]
    .map(({ itemIds, ...leader }) => ({
      ...leader,
      itemCount: itemIds.size,
    }))
    .sort((left, right) => (
      right.durationSeconds - left.durationSeconds
      || right.segmentCount - left.segmentCount
      || right.itemCount - left.itemCount
      || left.label.localeCompare(right.label)
    ));

  return {
    transcriptCharacterCount,
    speakers,
  };
}

class DashboardService {
  async getFastSnapshot(): Promise<DashboardSnapshot> {
    const [historyItems, projectCount, llmUsage] = await Promise.all([
      readHistoryItems(),
      readProjectCount(),
      llmUsageService.getStats(),
    ]);

    return {
      content: {
        overview: createOverview(historyItems, projectCount, false),
        speakers: null,
      },
      llmUsage,
      generatedAt: new Date().toISOString(),
    };
  }

  async getDeepSnapshot(): Promise<DashboardSnapshot> {
    const [historyItems, projectCount, llmUsage] = await Promise.all([
      readHistoryItems(),
      readProjectCount(),
      llmUsageService.getStats(),
    ]);

    const overview = createOverview(historyItems, projectCount, true);
    const transcriptAnalytics = await aggregateTranscriptAnalytics(historyItems);

    overview.transcriptCharacterCount = transcriptAnalytics.transcriptCharacterCount;

    return {
      content: {
        overview,
        speakers: transcriptAnalytics.speakers,
      },
      llmUsage,
      generatedAt: new Date().toISOString(),
    };
  }
}

export const dashboardService = new DashboardService();
