import { describe, expect, it } from 'vitest';
import type { TranscriptSegment } from '../../types/transcript';
import {
  buildTranscriptDiffRows,
  restoreSelectedTranscriptDiffRows,
} from '../transcriptDiff';

function segment(id: string, start: number, text: string, translation?: string): TranscriptSegment {
  return {
    id,
    start,
    end: start + 1,
    text,
    isFinal: true,
    ...(translation ? { translation } : {}),
  };
}

describe('transcriptDiff', () => {
  it('detects text and translation changes by segment id', () => {
    const rows = buildTranscriptDiffRows(
      [segment('1', 0, 'hello'), segment('2', 1, 'world', '世界')],
      [segment('1', 0, 'Hello'), segment('2', 1, 'world', '地球')],
    );

    expect(rows.map((row) => row.status)).toEqual(['modified', 'modified']);
  });

  it('matches re-transcribed segments by time and order when ids change', () => {
    const rows = buildTranscriptDiffRows(
      [segment('old-1', 0, 'hello'), segment('old-2', 2, 'world')],
      [segment('new-1', 0.1, 'hello'), segment('new-2', 2.1, 'World')],
    );

    expect(rows).toEqual([
      expect.objectContaining({ status: 'modified', snapshotIndex: 0, currentIndex: 0 }),
      expect.objectContaining({ status: 'modified', snapshotIndex: 1, currentIndex: 1 }),
    ]);
  });

  it('restores selected modified, removed, and added rows', () => {
    const rows = buildTranscriptDiffRows(
      [segment('1', 0, 'old'), segment('2', 1, 'removed')],
      [segment('1', 0, 'new'), segment('3', 2, 'added')],
    );
    const selected = new Set(rows.map((row) => row.id));

    const restored = restoreSelectedTranscriptDiffRows(rows, selected);

    expect(restored.map((item) => item.id)).toEqual(['1', '2']);
    expect(restored.map((item) => item.text)).toEqual(['old', 'removed']);
  });

  it('reports added and removed rows when segment counts diverge', () => {
    expect(buildTranscriptDiffRows(
      [segment('1', 0, 'one')],
      [segment('1', 0, 'one'), segment('2', 1, 'two')],
    ).map((row) => row.status)).toEqual(['unchanged', 'added']);

    expect(buildTranscriptDiffRows(
      [segment('1', 0, 'one'), segment('2', 1, 'two')],
      [segment('1', 0, 'one')],
    ).map((row) => row.status)).toEqual(['unchanged', 'removed']);
  });
});
