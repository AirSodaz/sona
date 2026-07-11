import type {
  TranscriptSegment,
  TranscriptTiming,
  TranscriptTimingSource,
  TranscriptTimingUnit,
  TranscriptUpdate,
} from '../types/transcript';
import { normalizeSpeakerAttribution } from '../types/speaker';
import { alignTextToTimedTokens } from './transcriptTextUtils';

function toSafeNumber(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function clampTime(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}

function doTimingUnitsMatchSegmentText(units: TranscriptTimingUnit[], text: string): boolean {
  const unitsText = units.map((unit) => unit.text).join('');
  return unitsText === text;
}

function normalizeUnitBoundaries(
  units: TranscriptTimingUnit[],
  start: number,
  end: number,
): TranscriptTimingUnit[] {
  if (units.length === 0) {
    return [];
  }

  const safeStart = Math.max(0, start);
  const safeEnd = Math.max(safeStart, end);

  return units
    .map((unit, index) => {
      const unitStart = clampTime(toSafeNumber(unit.start, safeStart), safeStart, safeEnd);
      const fallbackEnd = index === units.length - 1 ? safeEnd : unitStart;
      const unitEnd = clampTime(
        Math.max(unitStart, toSafeNumber(unit.end, fallbackEnd)),
        unitStart,
        safeEnd,
      );

      return {
        text: typeof unit.text === 'string' ? unit.text : '',
        start: unitStart,
        end: unitEnd,
      };
    })
    .filter((unit) => unit.text.length > 0);
}

function buildSegmentLevelTiming(segment: TranscriptSegment, source: TranscriptTimingSource): TranscriptTiming {
  return {
    level: 'segment',
    source,
    units: [{
      text: segment.text || '',
      start: segment.start,
      end: segment.end,
    }],
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function buildRawTokenWindows(
  timestamps: number[],
  durations: number[] | undefined,
  segmentEnd: number,
): Array<{ start: number; end: number }> {
  return timestamps.map((timestamp, index) => {
    const start = timestamp;
    const nextStart = timestamps[index + 1];
    const explicitEnd = durations && Number.isFinite(durations[index])
      ? start + Math.max(0, durations[index]!)
      : undefined;
    const end = Math.max(
      start,
      explicitEnd ?? (typeof nextStart === 'number' ? nextStart : segmentEnd),
    );

    return { start, end };
  });
}

function buildAlignedTimingUnits(
  text: string,
  rawUnits: Array<{ text: string; start: number; end: number }>,
): TranscriptTimingUnit[] {
  const safeText = typeof text === 'string' ? text : String(text || '');
  if (!safeText || rawUnits.length === 0) {
    return [];
  }

  return alignTextToTimedTokens(
    safeText,
    rawUnits.map((unit) => ({
      text: unit.text,
      timing: {
        start: unit.start,
        end: unit.end,
      },
    })),
  ).map((unit) => ({
    text: unit.text,
    start: unit.timing.start,
    end: unit.timing.end,
  }));
}

function buildTokenLevelTiming(segment: TranscriptSegment, source: TranscriptTimingSource): TranscriptTiming | undefined {
  const { tokens, timestamps } = segment;
  if (!Array.isArray(tokens) || !Array.isArray(timestamps) || tokens.length === 0 || tokens.length !== timestamps.length) {
    return undefined;
  }

  const durations = Array.isArray(segment.durations) && segment.durations.length === tokens.length
    ? segment.durations.map((duration) => Math.max(0, toSafeNumber(duration, 0)))
    : undefined;
  const rawUnits = buildRawTokenWindows(
    timestamps.map((timestamp) => toSafeNumber(timestamp, segment.start)),
    durations,
    segment.end,
  ).map((window, index) => ({
    text: tokens[index] || '',
    start: window.start,
    end: window.end,
  }));

  const alignedUnits = buildAlignedTimingUnits(segment.text, rawUnits);
  if (alignedUnits.length === 0) {
    return undefined;
  }

  return {
    level: 'token',
    source,
    units: normalizeUnitBoundaries(alignedUnits, segment.start, segment.end),
  };
}

function coerceTiming(segment: TranscriptSegment): TranscriptTiming {
  const timing = segment.timing;
  if (timing && (timing.level === 'token' || timing.level === 'segment') && (timing.source === 'model' || timing.source === 'derived')) {
    if (timing.level === 'segment') {
      return {
        ...buildSegmentLevelTiming(segment, timing.source),
      };
    }

    const normalizedUnits = normalizeUnitBoundaries(
      Array.isArray(timing.units) ? timing.units : [],
      segment.start,
      segment.end,
    );
    if (normalizedUnits.length > 0) {
      if (!doTimingUnitsMatchSegmentText(normalizedUnits, segment.text)) {
        return buildTokenLevelTiming(segment, 'model') ?? buildSegmentLevelTiming(segment, 'derived');
      }

      return {
        level: timing.level,
        source: timing.source,
        units: normalizedUnits,
      };
    }
  }

  return buildTokenLevelTiming(segment, 'model') ?? buildSegmentLevelTiming(segment, 'derived');
}

export function normalizeTranscriptSegment(segment: TranscriptSegment): TranscriptSegment {
  const safeStart = Math.max(0, toSafeNumber(segment.start, 0));
  const safeEnd = Math.max(safeStart, toSafeNumber(segment.end, safeStart));
  const normalizedBase: TranscriptSegment = {
    ...segment,
    text: typeof segment.text === 'string' ? segment.text : '',
    start: safeStart,
    end: safeEnd,
    isFinal: segment.isFinal !== false,
  };

  return {
    ...normalizedBase,
    timing: coerceTiming(normalizedBase),
    speakerAttribution: normalizeSpeakerAttribution(segment.speakerAttribution),
  };
}

export function normalizeTranscriptSegments(segments: TranscriptSegment[]): TranscriptSegment[] {
  return segments.map((segment) => normalizeTranscriptSegment(segment));
}

export function shiftTranscriptSegment(segment: TranscriptSegment, offsetSeconds: number): TranscriptSegment {
  if (!Number.isFinite(offsetSeconds) || offsetSeconds === 0) {
    return segment;
  }

  return normalizeTranscriptSegment({
    ...segment,
    start: segment.start + offsetSeconds,
    end: segment.end + offsetSeconds,
    timestamps: segment.timestamps?.map((timestamp) => timestamp + offsetSeconds),
    timing: segment.timing
      ? {
          ...segment.timing,
          units: segment.timing.units.map((unit) => ({
            ...unit,
            start: unit.start + offsetSeconds,
            end: unit.end + offsetSeconds,
          })),
        }
      : segment.timing,
  });
}

export function normalizeTranscriptUpdate(update: TranscriptUpdate | TranscriptSegment | unknown): TranscriptUpdate {
  if (isRecord(update) && ('removeIds' in update || 'upsertSegments' in update)) {
    return {
      removeIds: Array.isArray(update.removeIds)
        ? update.removeIds.filter((id): id is string => typeof id === 'string' && id.length > 0)
        : [],
      upsertSegments: normalizeTranscriptSegments(
        Array.isArray(update.upsertSegments) ? update.upsertSegments : [],
      ),
    };
  }

  if (isRecord(update)) {
    return {
      removeIds: [],
      upsertSegments: [normalizeTranscriptSegment(update as unknown as TranscriptSegment)],
    };
  }

  return {
    removeIds: [],
    upsertSegments: [],
  };
}
