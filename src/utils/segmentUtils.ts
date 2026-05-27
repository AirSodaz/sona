import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment, TranscriptTimingUnit } from '../types/transcript';
import { alignTextToTimedTokens, stripHtmlTags } from './transcriptTextUtils';

// Constants
const MAX_SEGMENT_LENGTH_CJK = 36;
const MAX_SEGMENT_LENGTH_WESTERN = 84;

// Common abbreviations that shouldn't trigger a sentence split
const ABBREVIATIONS = new Set([
    'mr', 'mrs', 'ms', 'dr', 'prof', 'sr', 'jr', 'st', 'vs', 'etc',
    'no', 'op', 'vol', 'fig', 'inc', 'ltd', 'co', 'dept'
]);

// Regular expressions for text processing
const SPLIT_REGEX = /([.?!。？！]+)/;
const COMMA_SPLIT_REGEX = /([,，;；:：]+)/;
const PUNCTUATION_REGEX = /[\s\p{P}]/u;
const PUNCTUATION_REPLACE_REGEX = /[\s\p{P}]+/gu;

// Regex for detection
const CJK_REGEX = /[\p{sc=Han}\p{sc=Hiragana}\p{sc=Katakana}\p{sc=Hangul}]/u;

export { stripHtmlTags };

/**
 * Calculates the length of the text excluding punctuation and whitespace.
 */
function getEffectiveLength(text: string): number {
    if (!PUNCTUATION_REGEX.test(text)) {
        return text.length;
    }

    let reduction = 0;
    PUNCTUATION_REPLACE_REGEX.lastIndex = 0;
    let match;
    while ((match = PUNCTUATION_REPLACE_REGEX.exec(text)) !== null) {
        reduction += match[0].length;
    }
    return text.length - reduction;
}

/**
 * Checks if the given text ends with a known abbreviation.
 */
function endsWithAbbreviation(text: string): boolean {
    const trimmed = text.trim();
    if (!trimmed) return false;
    const words = trimmed.split(/\s+/);
    const lastWord = words[words.length - 1].toLowerCase();
    return ABBREVIATIONS.has(lastWord);
}

/**
 * Determines if text contains CJK characters to decide on segment length limit.
 */
function isCJK(text: string): boolean {
    return CJK_REGEX.test(text);
}

/**
 * Splits transcript segments based on punctuation marks.
 */
export function splitByPunctuation(segments: TranscriptSegment[]): TranscriptSegment[] {
    // Pass 1: Sentence Splitting (Strong Punctuation)
    const intermediateSegments: TranscriptSegment[] = [];

    for (const segment of segments) {
        const parts = splitSegmentByRegex(segment, SPLIT_REGEX, { checkAbbreviations: true });
        intermediateSegments.push(...parts);
    }

    // Pass 2: Length Constraints (Weak Punctuation)
    const finalSegments: TranscriptSegment[] = [];

    for (const segment of intermediateSegments) {
        const segmentText = typeof segment.text === 'string' ? segment.text : String(segment.text || '');
        const limit = isCJK(segmentText) ? MAX_SEGMENT_LENGTH_CJK : MAX_SEGMENT_LENGTH_WESTERN;

        if (segmentText.length > limit) {
            const subSegments = splitSegmentByRegex(segment, COMMA_SPLIT_REGEX, { checkAbbreviations: false });
            finalSegments.push(...subSegments);
        } else {
            finalSegments.push(segment);
        }
    }

    return finalSegments;
}

interface SplitOptions {
    checkAbbreviations: boolean;
}

/** State for the splitting process */
interface SplitterState {
    currentText: string;
    currentStart: number; // Time
    currentSegmentStart: number; // Time
    charIndex: number;
    effectiveCharIndex: number;
    lastTokenIndex: number;
    nextTokenSliceStart: number;
}

/**
 * Helper to split a single segment by a given regex pattern.
 */
function splitSegmentByRegex(segment: TranscriptSegment, regex: RegExp, options: SplitOptions): TranscriptSegment[] {
    const text = typeof segment.text === 'string' ? segment.text : String(segment.text || '');
    const parts = text.split(regex);

    if (parts.length <= 1) {
        return [{ ...segment, id: uuidv4() }];
    }

    const hasTimestamps = checkHasTimestamps(segment);
    const tokenMap = hasTimestamps ? buildTokenMap(segment) : null;
    const totalDuration = segment.end - segment.start;
    const totalLength = text.length;

    // Initialize state
    const state: SplitterState = {
        currentText: "",
        currentStart: segment.start,
        currentSegmentStart: (hasTimestamps && tokenMap && tokenMap.timestamps.length > 0)
            ? tokenMap.timestamps[0]
            : segment.start,
        charIndex: 0,
        effectiveCharIndex: 0,
        lastTokenIndex: 0,
        nextTokenSliceStart: 0,
    };

    const newSegments: TranscriptSegment[] = [];

    for (let i = 0; i < parts.length; i++) {
        const part = parts[i];
        const partEffectiveLen = getEffectiveLength(part);

        if (regex.test(part)) {
            handleDelimiter(part, partEffectiveLen, state, options, hasTimestamps, tokenMap, segment, totalLength, totalDuration, newSegments);
        } else {
            // Content
            state.currentText += part;
            state.charIndex += part.length;
            state.effectiveCharIndex += partEffectiveLen;
        }
    }

    // Leftover text
    if (state.currentText.trim()) {
        finalizeSegment(state, segment, hasTimestamps, tokenMap, newSegments);
    }

    return newSegments;
}

function checkHasTimestamps(segment: TranscriptSegment): boolean {
    return !!(segment.tokens && segment.timestamps && segment.tokens.length === segment.timestamps.length);
}

function handleDelimiter(
    part: string,
    partEffectiveLen: number,
    state: SplitterState,
    options: SplitOptions,
    hasTimestamps: boolean,
    tokenMap: TokenMap | null,
    originalSegment: TranscriptSegment,
    totalLength: number,
    totalDuration: number,
    results: TranscriptSegment[]
) {
    // Check abbreviation
    let shouldMerge = false;
    if (options.checkAbbreviations && part.includes('.')) {
        if (endsWithAbbreviation(state.currentText)) {
            shouldMerge = true;
        }
    }

    if (shouldMerge) {
        state.currentText += part;
        state.charIndex += part.length;
        state.effectiveCharIndex += partEffectiveLen;
        return;
    }

    // Normal split
    state.currentText += part;
    state.charIndex += part.length;
    state.effectiveCharIndex += partEffectiveLen;

    // Calculate segment end and tokens
    let segmentEnd: number;
    let currentTokens: string[] | undefined;
    let currentTimestamps: number[] | undefined;
    let currentDurations: number[] | undefined;

    const fallbackSegmentEnd = state.currentStart + (totalLength > 0 ? (state.currentText.length / totalLength) * totalDuration : 0);

    if (hasTimestamps && tokenMap) {
        const found = findTimestampFromMap(tokenMap, state.effectiveCharIndex, state.lastTokenIndex);
        let sliceEnd = tokenMap.timestamps.length;

        if (found) {
            sliceEnd = found.index;
            segmentEnd = found.timestamp;
            state.lastTokenIndex = found.index;
        } else {
            segmentEnd = fallbackSegmentEnd;
        }

        if (sliceEnd > state.nextTokenSliceStart) {
            currentTokens = originalSegment.tokens!.slice(state.nextTokenSliceStart, sliceEnd);
            currentTimestamps = originalSegment.timestamps!.slice(state.nextTokenSliceStart, sliceEnd);
            if (originalSegment.durations && originalSegment.durations.length === originalSegment.tokens!.length) {
                currentDurations = originalSegment.durations.slice(state.nextTokenSliceStart, sliceEnd);
            }
            state.nextTokenSliceStart = sliceEnd;
        }

        if (currentTimestamps && currentTimestamps.length > 0) {
            state.currentSegmentStart = currentTimestamps[0];
        }
    } else {
        segmentEnd = fallbackSegmentEnd;
    }

    results.push({
        id: uuidv4(),
        text: state.currentText.trim(),
        start: state.currentSegmentStart,
        end: segmentEnd,
        isFinal: true,
        tokens: currentTokens,
        timestamps: currentTimestamps,
        durations: currentDurations,
        translation: originalSegment.translation,
        speaker: originalSegment.speaker,
    });

    // Prepare for next
    state.currentStart = segmentEnd;
    state.currentSegmentStart = segmentEnd;
    state.currentText = "";

    // Adjust next start if tokens available
    if (hasTimestamps && tokenMap && state.lastTokenIndex < tokenMap.timestamps.length) {
        if (state.lastTokenIndex === state.nextTokenSliceStart) {
            if (state.nextTokenSliceStart < tokenMap.timestamps.length) {
                state.currentSegmentStart = tokenMap.timestamps[state.nextTokenSliceStart];
                state.currentStart = state.currentSegmentStart;
            }
        }
    }
}

function finalizeSegment(
    state: SplitterState,
    originalSegment: TranscriptSegment,
    hasTimestamps: boolean,
    tokenMap: TokenMap | null,
    results: TranscriptSegment[]
) {
    const segmentEnd = originalSegment.end;
    let currentTokens: string[] | undefined;
    let currentTimestamps: number[] | undefined;
    let currentDurations: number[] | undefined;

    if (hasTimestamps && tokenMap) {
        currentTokens = originalSegment.tokens!.slice(state.nextTokenSliceStart);
        currentTimestamps = originalSegment.timestamps!.slice(state.nextTokenSliceStart);
        if (originalSegment.durations && originalSegment.durations.length === originalSegment.tokens!.length) {
            currentDurations = originalSegment.durations.slice(state.nextTokenSliceStart);
        }

        if (currentTimestamps.length > 0) {
            state.currentSegmentStart = currentTimestamps[0];
        }
    }

    results.push({
        id: uuidv4(),
        text: state.currentText.trim(),
        start: state.currentSegmentStart,
        end: segmentEnd,
        isFinal: true,
        tokens: currentTokens,
        timestamps: currentTimestamps,
        durations: currentDurations,
        translation: originalSegment.translation,
        speaker: originalSegment.speaker,
    });
}

/** Pre-calculated map for faster lookups. */
interface TokenMap {
    startIndices: number[];
    endIndices: number[];
    timestamps: number[];
}

/**
 * Builds a map of tokens to their timestamps and effective indices.
 */
function buildTokenMap(segment: TranscriptSegment): TokenMap | null {
    if (!segment.tokens || !segment.timestamps) return null;

    const startIndices: number[] = [];
    const endIndices: number[] = [];
    const timestamps: number[] = [];

    let currentLen = 0;

    for (let i = 0; i < segment.tokens.length; i++) {
        const token = segment.tokens[i];
        const tokenLen = getEffectiveLength(typeof token === 'string' ? token : '');

        startIndices.push(currentLen);
        currentLen += tokenLen;
        endIndices.push(currentLen);
        timestamps.push(segment.timestamps[i]);
    }

    return { startIndices, endIndices, timestamps };
}

/**
 * Finds the timestamp corresponding to an effective character index from the map.
 */
function findTimestampFromMap(map: TokenMap, effectiveIndex: number, hintIndex: number = 0): { timestamp: number, index: number } | undefined {
    // Check hint first
    if (hintIndex < map.startIndices.length && map.startIndices[hintIndex] <= effectiveIndex && effectiveIndex < map.endIndices[hintIndex]) {
        return { timestamp: map.timestamps[hintIndex], index: hintIndex };
    }

    const next = hintIndex + 1;
    if (next < map.startIndices.length && map.startIndices[next] <= effectiveIndex && effectiveIndex < map.endIndices[next]) {
        return { timestamp: map.timestamps[next], index: next };
    }

    // Binary search
    let left = hintIndex;
    let right = map.startIndices.length - 1;

    if (left > right || (left < map.startIndices.length && map.startIndices[left] > effectiveIndex)) {
        left = 0;
    }

    let idx = -1;
    while (left <= right) {
        const mid = (left + right) >>> 1;
        if (map.startIndices[mid] <= effectiveIndex) {
            idx = mid;
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    if (idx === -1 || effectiveIndex >= map.endIndices[idx]) {
        return undefined;
    }

    return { timestamp: map.timestamps[idx], index: idx };
}

function checkHintIndex(segments: TranscriptSegment[], searchTime: number, time: number, hintIndex: number): { segment: TranscriptSegment | undefined, index: number } | null {
    if (hintIndex < -1 || hintIndex >= segments.length) {
        return null;
    }

    if (hintIndex === -1) {
        if (segments.length === 0) return null;
        if (time < segments[0].start) {
            return { segment: undefined, index: -1 };
        }
        if (time < segments[0].end && time >= segments[0].start) {
            return { segment: segments[0], index: 0 };
        }
        return null;
    }

    const seg = segments[hintIndex];
    if (seg.start <= searchTime && time < seg.end) {
        return { segment: seg, index: hintIndex };
    }

    const nextIdx = hintIndex + 1;
    if (nextIdx < segments.length) {
        const nextSeg = segments[nextIdx];
        if (nextSeg.start <= searchTime && time < nextSeg.end) {
            return { segment: nextSeg, index: nextIdx };
        }
    }

    if (time >= seg.end && (nextIdx >= segments.length || searchTime < segments[nextIdx].start)) {
        return { segment: undefined, index: hintIndex };
    }

    const prevIdx = hintIndex - 1;
    if (prevIdx >= 0) {
        const prevSeg = segments[prevIdx];
        if (prevSeg.start <= searchTime && time < prevSeg.end) {
            return { segment: prevSeg, index: prevIdx };
        }

        if (time >= prevSeg.end && searchTime < seg.start) {
            return { segment: undefined, index: prevIdx };
        }
    }

    return null;
}

/**
 * Efficiently finds the segment containing the given time using binary search.
 */
export function findSegmentAndIndexForTime(
    segments: TranscriptSegment[],
    time: number,
    hintIndex?: number
): { segment: TranscriptSegment | undefined, index: number } {
    const EPSILON = 0.05;
    const searchTime = time + EPSILON;

    if (hintIndex !== undefined) {
        const hintResult = checkHintIndex(segments, searchTime, time, hintIndex);
        if (hintResult !== null) {
            return hintResult;
        }
    }

    let left = 0;
    let right = segments.length - 1;
    let idx = -1;

    while (left <= right) {
        const mid = (left + right) >>> 1;
        if (segments[mid].start <= searchTime) {
            idx = mid;
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    if (idx !== -1 && time <= segments[idx].end) {
        return { segment: segments[idx], index: idx };
    }

    return { segment: undefined, index: idx };
}

export function findSegmentForTime(segments: TranscriptSegment[], time: number): TranscriptSegment | undefined {
    return findSegmentAndIndexForTime(segments, time).segment;
}

function fingerprintSpeakerAttribution(segment: TranscriptSegment): string {
    const attribution = segment.speakerAttribution;
    if (!attribution) {
        return '';
    }

    const candidates = attribution.candidates
        .map((candidate) => (
            `${candidate.profileId}:${candidate.profileName}:${candidate.score}:${candidate.rank}`
        ))
        .join(',');

    return [
        attribution.groupId,
        attribution.anonymousLabel,
        attribution.state,
        attribution.source,
        attribution.confidence,
        candidates,
    ].join(':');
}

export function computeSegmentsFingerprint(segments: TranscriptSegment[]): string {
    return segments.map(s =>
        `${s.id}:${s.text}:${s.start}:${s.end}:${s.isFinal}:${s.translation || ''}:${s.speaker?.id || ''}:${s.speaker?.label || ''}:${s.speaker?.kind || ''}:${s.speaker?.score || ''}:${fingerprintSpeakerAttribution(s)}`
    ).join('|');
}

export function computeSummarySourceFingerprint(segments: TranscriptSegment[]): string {
    return segments.map(s =>
        `${s.id}:${s.text}:${s.start}:${s.end}:${s.isFinal}:${s.speaker?.id || ''}:${s.speaker?.label || ''}:${s.speaker?.kind || ''}:${s.speaker?.score || ''}`
    ).join('|');
}

export function alignTokensToText(
    text: string,
    rawTokens: string[],
    rawTimestamps: number[]
): { text: string; timestamp: number }[] {
    const safeText = typeof text === 'string' ? text : String(text || '');
    if (!safeText || !Array.isArray(rawTokens) || !Array.isArray(rawTimestamps) || rawTokens.length !== rawTimestamps.length) {
        return [{ text: safeText, timestamp: rawTimestamps?.[0] || 0 }];
    }

    return alignTextToTimedTokens(
        safeText,
        rawTokens.map((token, index) => ({
            text: token,
            timing: rawTimestamps[index] || 0,
        })),
    ).map((unit) => ({
        text: unit.text,
        timestamp: unit.timing,
    }));
}

/**
 * Pure mathematical timing/token split calculation for a single TranscriptSegment.
 * Resolves both modern timing units and legacy token arrays, then constructs
 * the resulting left and right segment halves.
 */
export function performSegmentSplit(
  segment: TranscriptSegment,
  caretOffset: number,
  plainText: string,
  leftText: string,
  rightText: string,
  newSegmentId: string
): { segmentLeft: TranscriptSegment; segmentRight: TranscriptSegment } {
  const totalLength = plainText.length;

  const leftUnits: TranscriptTimingUnit[] = [];
  const rightUnits: TranscriptTimingUnit[] = [];
  let splitTime = segment.start;
  let splitTimeFound = false;

  if (segment.timing && segment.timing.units && segment.timing.units.length > 0) {
    const units = segment.timing.units;
    let cumulativeLen = 0;
    for (const unit of units) {
      cumulativeLen += stripHtmlTags(unit.text).length;
      if (cumulativeLen <= caretOffset) {
        leftUnits.push(unit);
      } else {
        rightUnits.push(unit);
      }
    }
    if (rightUnits.length > 0) {
      splitTime = rightUnits[0].start;
      splitTimeFound = true;
    } else if (leftUnits.length > 0) {
      splitTime = leftUnits[leftUnits.length - 1].end;
      splitTimeFound = true;
    }
  }

  const leftTokens: string[] = [];
  const rightTokens: string[] = [];
  const leftTimestamps: number[] = [];
  const rightTimestamps: number[] = [];
  const leftDurations: number[] = [];
  const rightDurations: number[] = [];

  const hasLegacyTimestamps = Boolean(
    segment.tokens &&
    segment.timestamps &&
    segment.tokens.length > 0 &&
    segment.tokens.length === segment.timestamps.length
  );

  if (hasLegacyTimestamps && segment.tokens && segment.timestamps) {
    let cumulativeLen = 0;
    for (let i = 0; i < segment.tokens.length; i++) {
      const token = segment.tokens[i];
      cumulativeLen += stripHtmlTags(token).length;
      if (cumulativeLen <= caretOffset) {
        leftTokens.push(token);
        leftTimestamps.push(segment.timestamps[i]);
        if (segment.durations) leftDurations.push(segment.durations[i]);
      } else {
        rightTokens.push(token);
        rightTimestamps.push(segment.timestamps[i]);
        if (segment.durations) rightDurations.push(segment.durations[i]);
      }
    }
    if (!splitTimeFound) {
      if (rightTimestamps.length > 0) {
        splitTime = rightTimestamps[0];
        splitTimeFound = true;
      } else if (leftTimestamps.length > 0 && segment.durations && leftDurations.length > 0) {
        splitTime = leftTimestamps[leftTimestamps.length - 1] + leftDurations[leftDurations.length - 1];
        splitTimeFound = true;
      }
    }
  }

  if (!splitTimeFound) {
    const ratio = totalLength > 0 ? Math.min(1, Math.max(0, caretOffset / totalLength)) : 0.5;
    const duration = segment.end - segment.start;
    splitTime = Math.round((segment.start + ratio * duration) * 100) / 100;
  }

  // Bound splitTime safety
  splitTime = Math.min(segment.end, Math.max(segment.start, splitTime));

  const segmentLeft: TranscriptSegment = {
    ...segment,
    end: splitTime,
    text: leftText,
    timing: segment.timing ? {
      ...segment.timing,
      units: leftUnits,
    } : undefined,
    tokens: segment.tokens ? leftTokens : undefined,
    timestamps: segment.timestamps ? leftTimestamps : undefined,
    durations: segment.durations ? leftDurations : undefined,
  };

  const segmentRight: TranscriptSegment = {
    id: newSegmentId,
    start: splitTime,
    end: segment.end,
    text: rightText,
    isFinal: true,
    speaker: segment.speaker,
    speakerAttribution: segment.speakerAttribution,
    timing: segment.timing ? {
      ...segment.timing,
      units: rightUnits,
    } : undefined,
    tokens: segment.tokens ? rightTokens : undefined,
    timestamps: segment.timestamps ? rightTimestamps : undefined,
    durations: segment.durations ? rightDurations : undefined,
  };

  return { segmentLeft, segmentRight };
}
