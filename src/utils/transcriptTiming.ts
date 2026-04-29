import type {
  TranscriptSegment,
  TranscriptTiming,
  TranscriptTimingSource,
  TranscriptTimingUnit,
  TranscriptUpdate,
} from '../types/transcript';

const NORMALIZE_REGEX = /[^\p{L}\p{N}]/gu;
const PUNCTUATION_ONLY_REGEX = /^[^\p{L}\p{N}]+$/u;
const WHITESPACE_ONLY_REGEX = /^\s+$/;
const LEXER_REGEX = /(<\/?(?:b|i|u)>)|(\s+)|([\p{sc=Han}])|([^<\s\p{sc=Han}]+)|(<)/gui;

function toSafeNumber(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function clampTime(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}

function stripHtmlTags(text: string): string {
  return text.replace(/<\/?[^>]+(>|$)/g, '');
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

  const normalizedTokens = rawUnits.map((unit) => (
    typeof unit.text === 'string'
      ? unit.text.toLowerCase().replace(NORMALIZE_REGEX, '')
      : ''
  ));

  const words: string[] = [];
  const normalizedWords: string[] = [];
  const activeTags = new Set<string>();

  let match: RegExpExecArray | null;
  LEXER_REGEX.lastIndex = 0;
  while ((match = LEXER_REGEX.exec(safeText)) !== null) {
    const [full, tag] = match;

    if (tag) {
      const tagName = tag.replace(/[<\/>]/g, '').toLowerCase();
      if (tag.startsWith('</')) {
        activeTags.delete(tagName);
      } else {
        activeTags.add(tagName);
      }
      continue;
    }

    const tokenText = full;
    let wrapped = tokenText;
    const sortedTags = Array.from(activeTags).sort().reverse();
    for (const activeTag of sortedTags) {
      wrapped = `<${activeTag}>${wrapped}</${activeTag}>`;
    }

    const isPunctuation = PUNCTUATION_ONLY_REGEX.test(tokenText) && !WHITESPACE_ONLY_REGEX.test(tokenText);
    if (words.length > 0 && isPunctuation && !WHITESPACE_ONLY_REGEX.test(stripHtmlTags(words[words.length - 1]))) {
      words[words.length - 1] += wrapped;
      normalizedWords[normalizedWords.length - 1] = stripHtmlTags(words[words.length - 1]).toLowerCase().replace(NORMALIZE_REGEX, '');
      continue;
    }

    words.push(wrapped);
    normalizedWords.push(stripHtmlTags(wrapped).toLowerCase().replace(NORMALIZE_REGEX, ''));
  }

  let joinedTokens = '';
  const charToTokenIndex: number[] = [];
  for (let index = 0; index < normalizedTokens.length; index += 1) {
    const token = normalizedTokens[index];
    joinedTokens += token;
    for (let charIndex = 0; charIndex < token.length; charIndex += 1) {
      charToTokenIndex.push(index);
    }
  }

  const getFallbackUnit = (charPos: number) => {
    if (rawUnits.length === 0) {
      return null;
    }
    if (charToTokenIndex.length === 0) {
      return rawUnits[rawUnits.length - 1];
    }
    if (charPos >= charToTokenIndex.length) {
      return rawUnits[charToTokenIndex[charToTokenIndex.length - 1]];
    }
    return rawUnits[charToTokenIndex[charPos]];
  };

  const result: TranscriptTimingUnit[] = [];
  let charPos = 0;
  const maxChars = joinedTokens.length;

  for (let index = 0; index < words.length; index += 1) {
    const word = words[index];
    const cleanWord = stripHtmlTags(word);
    const normalizedWord = normalizedWords[index];

    if (!cleanWord.trim() || !normalizedWord) {
      const fallback = getFallbackUnit(charPos);
      if (fallback) {
        result.push({ text: word, start: fallback.start, end: fallback.end });
      }
      continue;
    }

    const searchLimit = Math.max(20, normalizedWord.length * 2);
    const searchWindow = joinedTokens.substring(charPos, charPos + searchLimit);
    const localIndex = searchWindow.indexOf(normalizedWord);

    let fallback = getFallbackUnit(charPos);
    if (localIndex !== -1) {
      const matchPos = charPos + localIndex;
      fallback = getFallbackUnit(matchPos);
      charPos = matchPos + normalizedWord.length;
    } else {
      let nextNormalizedWord = '';
      for (let nextIndex = index + 1; nextIndex < words.length; nextIndex += 1) {
        nextNormalizedWord = normalizedWords[nextIndex];
        if (nextNormalizedWord) {
          break;
        }
      }

      if (nextNormalizedWord) {
        const nextSearchWindow = joinedTokens.substring(charPos, charPos + searchLimit + nextNormalizedWord.length);
        const nextLocalIndex = nextSearchWindow.indexOf(nextNormalizedWord);
        charPos = nextLocalIndex !== -1 ? charPos + nextLocalIndex : charPos + 1;
      } else {
        charPos = maxChars;
      }
    }

    if (fallback) {
      result.push({ text: word, start: fallback.start, end: fallback.end });
    }

    if (charPos > maxChars) {
      charPos = maxChars;
    }
  }

  return result;
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
      upsertSegments: [normalizeTranscriptSegment(update as TranscriptSegment)],
    };
  }

  return {
    removeIds: [],
    upsertSegments: [],
  };
}
