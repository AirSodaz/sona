import type { TranscriptSegment } from '../types/transcript';
import type { TranscriptDiffRow } from '../types/transcriptSnapshot';
import { normalizeTranscriptSegments } from './transcriptTiming';

const TIME_MATCH_TOLERANCE_SECONDS = 1.5;

function cloneSegment(segment: TranscriptSegment): TranscriptSegment {
  return JSON.parse(JSON.stringify(segment)) as TranscriptSegment;
}

function segmentSignature(segment: TranscriptSegment | undefined): string {
  if (!segment) {
    return '';
  }

  return JSON.stringify({
    end: segment.end,
    isFinal: segment.isFinal,
    speaker: segment.speaker || null,
    start: segment.start,
    text: segment.text || '',
    timing: segment.timing || null,
    translation: segment.translation || '',
  });
}

function isSameSegmentContent(
  snapshotSegment: TranscriptSegment | undefined,
  currentSegment: TranscriptSegment | undefined,
): boolean {
  return segmentSignature(snapshotSegment) === segmentSignature(currentSegment);
}

function segmentTimeDistance(snapshotSegment: TranscriptSegment, currentSegment: TranscriptSegment): number {
  return Math.abs(snapshotSegment.start - currentSegment.start)
    + Math.abs(snapshotSegment.end - currentSegment.end);
}

function findBestTimeMatch(
  snapshotSegment: TranscriptSegment,
  currentSegments: TranscriptSegment[],
  usedCurrentIndexes: ReadonlySet<number>,
): number | null {
  let bestIndex: number | null = null;
  let bestDistance = Number.POSITIVE_INFINITY;

  currentSegments.forEach((currentSegment, index) => {
    if (usedCurrentIndexes.has(index)) {
      return;
    }

    const distance = segmentTimeDistance(snapshotSegment, currentSegment);
    if (distance <= TIME_MATCH_TOLERANCE_SECONDS && distance < bestDistance) {
      bestDistance = distance;
      bestIndex = index;
    }
  });

  return bestIndex;
}

function buildRowId(snapshotIndex: number | null, currentIndex: number | null): string {
  return `diff-${snapshotIndex ?? 'x'}-${currentIndex ?? 'x'}`;
}

export function buildTranscriptDiffRows(
  snapshotSegmentsInput: TranscriptSegment[],
  currentSegmentsInput: TranscriptSegment[],
): TranscriptDiffRow[] {
  const snapshotSegments = normalizeTranscriptSegments(snapshotSegmentsInput);
  const currentSegments = normalizeTranscriptSegments(currentSegmentsInput);
  const currentIndexById = new Map<string, number>();
  const matches = new Map<number, number>();
  const usedCurrentIndexes = new Set<number>();

  currentSegments.forEach((segment, index) => {
    currentIndexById.set(segment.id, index);
  });

  snapshotSegments.forEach((snapshotSegment, snapshotIndex) => {
    const currentIndex = currentIndexById.get(snapshotSegment.id);
    if (currentIndex === undefined || usedCurrentIndexes.has(currentIndex)) {
      return;
    }

    matches.set(snapshotIndex, currentIndex);
    usedCurrentIndexes.add(currentIndex);
  });

  snapshotSegments.forEach((snapshotSegment, snapshotIndex) => {
    if (matches.has(snapshotIndex)) {
      return;
    }

    const currentIndex = findBestTimeMatch(snapshotSegment, currentSegments, usedCurrentIndexes);
    if (currentIndex === null) {
      return;
    }

    matches.set(snapshotIndex, currentIndex);
    usedCurrentIndexes.add(currentIndex);
  });

  const unmatchedSnapshotIndexes = snapshotSegments
    .map((_, index) => index)
    .filter((index) => !matches.has(index));
  const unmatchedCurrentIndexes = currentSegments
    .map((_, index) => index)
    .filter((index) => !usedCurrentIndexes.has(index));
  const orderMatchCount = Math.min(unmatchedSnapshotIndexes.length, unmatchedCurrentIndexes.length);

  for (let index = 0; index < orderMatchCount; index += 1) {
    matches.set(unmatchedSnapshotIndexes[index], unmatchedCurrentIndexes[index]);
    usedCurrentIndexes.add(unmatchedCurrentIndexes[index]);
  }

  const rows: TranscriptDiffRow[] = [];
  const finalMatchedCurrentIndexes = new Set<number>();

  snapshotSegments.forEach((snapshotSegment, snapshotIndex) => {
    const currentIndex = matches.get(snapshotIndex);
    if (currentIndex === undefined) {
      rows.push({
        id: buildRowId(snapshotIndex, null),
        status: 'removed',
        snapshotSegment,
        snapshotIndex,
        currentIndex: null,
      });
      return;
    }

    finalMatchedCurrentIndexes.add(currentIndex);
    const currentSegment = currentSegments[currentIndex];
    rows.push({
      id: buildRowId(snapshotIndex, currentIndex),
      status: isSameSegmentContent(snapshotSegment, currentSegment) ? 'unchanged' : 'modified',
      snapshotSegment,
      currentSegment,
      snapshotIndex,
      currentIndex,
    });
  });

  currentSegments.forEach((currentSegment, currentIndex) => {
    if (finalMatchedCurrentIndexes.has(currentIndex)) {
      return;
    }

    rows.push({
      id: buildRowId(null, currentIndex),
      status: 'added',
      currentSegment,
      snapshotIndex: null,
      currentIndex,
    });
  });

  return rows.sort((left, right) => {
    const leftIndex = left.currentIndex ?? left.snapshotIndex ?? 0;
    const rightIndex = right.currentIndex ?? right.snapshotIndex ?? 0;
    if (leftIndex !== rightIndex) {
      return leftIndex - rightIndex;
    }
    return left.id.localeCompare(right.id);
  });
}

export function restoreSelectedTranscriptDiffRows(
  rows: TranscriptDiffRow[],
  selectedRowIds: ReadonlySet<string>,
): TranscriptSegment[] {
  const nextSegments: TranscriptSegment[] = [];

  rows.forEach((row) => {
    const selected = selectedRowIds.has(row.id);
    if (selected) {
      if (row.status === 'added') {
        return;
      }

      if (row.snapshotSegment) {
        nextSegments.push(cloneSegment(row.snapshotSegment));
      }
      return;
    }

    if (row.currentSegment) {
      nextSegments.push(cloneSegment(row.currentSegment));
    }
  });

  return normalizeTranscriptSegments(nextSegments);
}

export function countChangedTranscriptDiffRows(rows: TranscriptDiffRow[]): number {
  return rows.filter((row) => row.status !== 'unchanged').length;
}
