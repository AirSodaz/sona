import { v4 as uuidv4 } from 'uuid';
import { TranscriptSegment } from '../types/transcript';

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

// Regex for alignTokensToText
const NORMALIZE_REGEX = /[^\p{L}\p{N}]/gu;
const PUNCTUATION_ONLY_REGEX = /^[^\p{L}\p{N}]+$/u;
const WHITESPACE_ONLY_REGEX = /^\s+$/;

// Tag-aware lexer regex
const LEXER_REGEX = /(<\/?(?:b|i|u)>)|(\s+)|([\p{sc=Han}])|([^<\s\p{sc=Han}]+)|(<)/gui;

/**
 * Strips HTML tags from a string.
 */
export function stripHtmlTags(text: string): string {
    return text.replace(/<\/?[^>]+(>|$)/g, "");
}

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
    const intermediateSegments = segments.flatMap(segment =>
        splitSegmentByRegex(segment, SPLIT_REGEX, { checkAbbreviations: true })
    );

    // Pass 2: Length Constraints (Weak Punctuation)
    return intermediateSegments.flatMap(segment => {
        const segmentText = typeof segment.text === 'string' ? segment.text : String(segment.text || '');
        const limit = isCJK(segmentText) ? MAX_SEGMENT_LENGTH_CJK : MAX_SEGMENT_LENGTH_WESTERN;

        if (segmentText.length > limit) {
            return splitSegmentByRegex(segment, COMMA_SPLIT_REGEX, { checkAbbreviations: false });
        }
        return [segment];
    });
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

/** Context passed to delimiter handler to reduce argument count */
interface SplitterContext {
    state: SplitterState;
    originalSegment: TranscriptSegment;
    hasTimestamps: boolean;
    tokenMap: TokenMap | null;
    totalLength: number;
    totalDuration: number;
    results: TranscriptSegment[];
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

    const results: TranscriptSegment[] = [];
    const context: SplitterContext = {
        state,
        originalSegment: segment,
        hasTimestamps,
        tokenMap,
        totalLength,
        totalDuration,
        results
    };

    for (const part of parts) {
        const partEffectiveLen = getEffectiveLength(part);

        if (regex.test(part)) {
            handleDelimiter(part, partEffectiveLen, context, options);
        } else {
            // Content
            state.currentText += part;
            state.charIndex += part.length;
            state.effectiveCharIndex += partEffectiveLen;
        }
    }

    // Leftover text
    if (state.currentText.trim()) {
        finalizeSegment(context);
    }

    return results;
}

function checkHasTimestamps(segment: TranscriptSegment): boolean {
    return !!(segment.tokens && segment.timestamps && segment.tokens.length === segment.timestamps.length);
}

function handleDelimiter(
    part: string,
    partEffectiveLen: number,
    context: SplitterContext,
    options: SplitOptions
) {
    const { state, results, hasTimestamps, tokenMap, originalSegment, totalLength, totalDuration } = context;

    // Check abbreviation
    if (options.checkAbbreviations && part.includes('.')) {
        if (endsWithAbbreviation(state.currentText)) {
            state.currentText += part;
            state.charIndex += part.length;
            state.effectiveCharIndex += partEffectiveLen;
            return;
        }
    }

    // Normal split
    state.currentText += part;
    state.charIndex += part.length;
    state.effectiveCharIndex += partEffectiveLen;

    // Calculate segment end and tokens
    let segmentEnd: number;
    let currentTokens: string[] | undefined;
    let currentTimestamps: number[] | undefined;

    if (hasTimestamps && tokenMap) {
        const found = findTimestampFromMap(tokenMap, state.effectiveCharIndex, state.lastTokenIndex);
        let sliceEnd = tokenMap.timestamps.length;

        if (found) {
            sliceEnd = found.index;
            segmentEnd = found.timestamp;
            state.lastTokenIndex = found.index;
        } else {
            segmentEnd = state.currentStart + (totalLength > 0 ? (state.currentText.length / totalLength) * totalDuration : 0);
        }

        if (sliceEnd > state.nextTokenSliceStart) {
            currentTokens = originalSegment.tokens!.slice(state.nextTokenSliceStart, sliceEnd);
            currentTimestamps = originalSegment.timestamps!.slice(state.nextTokenSliceStart, sliceEnd);
            state.nextTokenSliceStart = sliceEnd;
        }

        if (currentTimestamps && currentTimestamps.length > 0) {
            state.currentSegmentStart = currentTimestamps[0];
        }
    } else {
        segmentEnd = state.currentStart + (totalLength > 0 ? (state.currentText.length / totalLength) * totalDuration : 0);
    }

    results.push({
        id: uuidv4(),
        text: state.currentText.trim(),
        start: state.currentSegmentStart,
        end: segmentEnd,
        isFinal: true,
        tokens: currentTokens,
        timestamps: currentTimestamps
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

function finalizeSegment(context: SplitterContext) {
    const { state, results, hasTimestamps, tokenMap, originalSegment } = context;

    let currentTokens: string[] | undefined;
    let currentTimestamps: number[] | undefined;

    if (hasTimestamps && tokenMap) {
        currentTokens = originalSegment.tokens!.slice(state.nextTokenSliceStart);
        currentTimestamps = originalSegment.timestamps!.slice(state.nextTokenSliceStart);

        if (currentTimestamps.length > 0) {
            state.currentSegmentStart = currentTimestamps[0];
        }
    }

    results.push({
        id: uuidv4(),
        text: state.currentText.trim(),
        start: state.currentSegmentStart,
        end: originalSegment.end,
        isFinal: true,
        tokens: currentTokens,
        timestamps: currentTimestamps
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
    if (hintIndex < map.startIndices.length) {
        if (map.startIndices[hintIndex] <= effectiveIndex && effectiveIndex < map.endIndices[hintIndex]) {
            return { timestamp: map.timestamps[hintIndex], index: hintIndex };
        }
        const next = hintIndex + 1;
        if (next < map.startIndices.length) {
            if (map.startIndices[next] <= effectiveIndex && effectiveIndex < map.endIndices[next]) {
                return { timestamp: map.timestamps[next], index: next };
            }
        }
    }

    // Binary search
    let left = hintIndex;
    let right = map.startIndices.length - 1;
    let idx = -1;

    if (left > right || (left < map.startIndices.length && map.startIndices[left] > effectiveIndex)) {
        left = 0;
    }

    while (left <= right) {
        const mid = (left + right) >>> 1;
        if (map.startIndices[mid] <= effectiveIndex) {
            idx = mid;
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    if (idx !== -1) {
        if (effectiveIndex < map.endIndices[idx]) {
            return { timestamp: map.timestamps[idx], index: idx };
        }
    }
    return undefined;
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

    // Hint optimization: Check hint and neighbors
    if (hintIndex !== undefined && hintIndex >= 0 && hintIndex < segments.length) {
        // Check current hint
        const seg = segments[hintIndex];
        if (seg.start <= searchTime && time < seg.end) {
            return { segment: seg, index: hintIndex };
        }

        // Check next
        const nextIdx = hintIndex + 1;
        if (nextIdx < segments.length) {
            const nextSeg = segments[nextIdx];
            if (nextSeg.start <= searchTime && time < nextSeg.end) {
                return { segment: nextSeg, index: nextIdx };
            }
        }

        // Check previous
        const prevIdx = hintIndex - 1;
        if (prevIdx >= 0) {
            const prevSeg = segments[prevIdx];
            if (prevSeg.start <= searchTime && time < prevSeg.end) {
                return { segment: prevSeg, index: prevIdx };
            }
        }
    }

    // Binary search if hint failed
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

    if (idx !== -1) {
        const seg = segments[idx];
        if (time <= seg.end) {
            return { segment: seg, index: idx };
        }
        return { segment: undefined, index: idx };
    }

    return { segment: undefined, index: -1 };
}

export function findSegmentForTime(segments: TranscriptSegment[], time: number): TranscriptSegment | undefined {
    return findSegmentAndIndexForTime(segments, time).segment;
}

export function computeSegmentsFingerprint(segments: TranscriptSegment[]): string {
    return segments.map(s =>
        `${s.id}:${s.text}:${s.start}:${s.end}:${s.isFinal}:${s.translation || ''}`
    ).join('|');
}

/**
 * Parses text into words and normalized words for token matching, preserving HTML tags.
 */
function parseTextForAlignment(text: string): { words: string[], normalizedWords: string[] } {
    const words: string[] = [];
    const normalizedWords: string[] = [];
    const activeTags = new Set<string>();

    let match;
    LEXER_REGEX.lastIndex = 0;

    while ((match = LEXER_REGEX.exec(text)) !== null) {
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
        for (const t of sortedTags) {
            wrapped = `<${t}>${wrapped}</${t}>`;
        }

        const isPunctuation = PUNCTUATION_ONLY_REGEX.test(tokenText) && !WHITESPACE_ONLY_REGEX.test(tokenText);

        if (words.length > 0 && isPunctuation && !WHITESPACE_ONLY_REGEX.test(stripHtmlTags(words[words.length - 1]))) {
            words[words.length - 1] += wrapped;
            normalizedWords[normalizedWords.length - 1] = stripHtmlTags(words[words.length - 1]).toLowerCase().replace(NORMALIZE_REGEX, '');
        } else {
            words.push(wrapped);
            normalizedWords.push(stripHtmlTags(wrapped).toLowerCase().replace(NORMALIZE_REGEX, ''));
        }
    }

    return { words, normalizedWords };
}

export function alignTokensToText(
    text: string,
    rawTokens: string[],
    rawTimestamps: number[]
): { text: string; timestamp: number }[] {
    const safeText = typeof text === 'string' ? text : String(text || '');

    // Quick validation
    if (!safeText || !Array.isArray(rawTokens) || !Array.isArray(rawTimestamps) || rawTokens.length !== rawTimestamps.length) {
        return [{ text: safeText, timestamp: rawTimestamps?.[0] || 0 }];
    }

    const { words, normalizedWords } = parseTextForAlignment(safeText);
    const normalizedTokens = rawTokens.map(t => typeof t === 'string' ? t.toLowerCase().replace(NORMALIZE_REGEX, '') : '');
    const result: { text: string; timestamp: number }[] = [];

    let currentRawIndex = 0;
    const maxTokens = rawTokens.length;

    for (let i = 0; i < words.length; i++) {
        const word = words[i];
        const normWord = normalizedWords[i];
        const cleanWord = stripHtmlTags(word);

        // Skip empty/invisible words
        if (!cleanWord.trim() || !normWord) {
            const ts = rawTimestamps.length > 0 ? rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)] : 0;
            result.push({ text: word, timestamp: ts });
            continue;
        }

        const startTimestamp = rawTimestamps.length > 0
            ? rawTimestamps[Math.min(currentRawIndex, rawTimestamps.length - 1)]
            : 0;
        result.push({ text: word, timestamp: startTimestamp });

        // Try to match tokens
        const matchResult = matchToken(normWord, normalizedTokens, currentRawIndex, maxTokens);

        if (matchResult.matched) {
            currentRawIndex = matchResult.nextIndex;
        } else {
            // Mismatch recovery: Look ahead for the next word
            const recoveryIndex = attemptRecovery(i, words, normalizedWords, normalizedTokens, currentRawIndex, maxTokens);

            if (recoveryIndex !== -1) {
                currentRawIndex = recoveryIndex;
            } else {
                currentRawIndex++;
            }
        }

        if (currentRawIndex >= maxTokens) currentRawIndex = maxTokens;
    }

    return result;
}

/**
 * Tries to match a normalized word against a sequence of normalized tokens.
 */
function matchToken(normWord: string, normalizedTokens: string[], currentIndex: number, maxTokens: number): { matched: boolean, nextIndex: number } {
    let accumulatedTokenStr = "";
    let tokensConsumed = 0;

    for (let j = 0; j < 5 && (currentIndex + j) < maxTokens; j++) {
        const t = normalizedTokens[currentIndex + j];
        accumulatedTokenStr += t;
        tokensConsumed++;

        if (accumulatedTokenStr.startsWith(normWord) || normWord.startsWith(accumulatedTokenStr)) {
            if (accumulatedTokenStr.length >= normWord.length) {
                return { matched: true, nextIndex: currentIndex + tokensConsumed };
            }
        }
    }
    return { matched: false, nextIndex: currentIndex };
}

/**
 * Attempts to recover synchronization by looking ahead.
 */
function attemptRecovery(
    currentWordIndex: number,
    words: string[],
    normalizedWords: string[],
    normalizedTokens: string[],
    currentRawIndex: number,
    maxTokens: number
): number {
    // Find next valid word to anchor to
    let nextNorm = "";
    for (let nextIdx = currentWordIndex + 1; nextIdx < words.length; nextIdx++) {
        nextNorm = normalizedWords[nextIdx];
        if (nextNorm) break;
    }

    if (nextNorm) {
        // Look ahead in tokens for this next word
        for (let k = 1; k < 10 && (currentRawIndex + k) < maxTokens; k++) {
            const t = normalizedTokens[currentRawIndex + k];
            if (t && t.startsWith(nextNorm)) {
                return currentRawIndex + k;
            }
        }
    } else {
        // If no more valid words, assume we can skip to end if near end of word list
        let hasRemainingContent = false;
        for (let r = currentWordIndex + 1; r < words.length; r++) {
            if (normalizedWords[r]) {
                hasRemainingContent = true;
                break;
            }
        }
        if (currentWordIndex === words.length - 1 || (currentWordIndex > words.length - 3 && !hasRemainingContent)) {
            return maxTokens;
        }
    }

    return -1;
}
