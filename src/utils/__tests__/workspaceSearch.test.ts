import { describe, expect, it } from 'vitest';
import {
  buildWorkspaceSearchSnippet,
  findWorkspaceSearchMatchRange,
  matchWorkspaceItem,
  normalizeWorkspaceSearchText,
} from '../workspaceSearch';

describe('workspaceSearch', () => {
  it('normalizes full-width characters, punctuation, and whitespace conservatively', () => {
    expect(normalizeWorkspaceSearchText('  ＡＢＣ，\n\tHello　World  ').text).toBe('abc, hello world');
  });

  it('matches Chinese punctuation against ASCII punctuation', () => {
    const normalizedQuery = normalizeWorkspaceSearchText('你好,世界').text;
    expect(findWorkspaceSearchMatchRange('你好，世界', normalizedQuery)).toEqual({ start: 0, end: 5 });
  });

  it('does not aggressively match across whitespace gaps', () => {
    const result = matchWorkspaceItem(
      {
        title: 'Quarterly notes',
        previewText: 'hello world',
        searchContent: 'hello world',
      },
      'helloworld',
    );

    expect(result).toBeNull();
  });

  it('prefers body-side snippets even when the title also matches', () => {
    const result = matchWorkspaceItem(
      {
        title: 'Roadmap Review',
        previewText: 'Quarterly roadmap discussion with design and product.',
        searchContent: 'Quarterly roadmap discussion with design and product.',
      },
      'roadmap',
    );

    expect(result?.matchedField).toBe('title');
    expect(result?.titleMatch).toEqual({ start: 0, end: 7 });
    expect(result?.displaySnippet.text).toContain('Quarterly roadmap discussion');
  });

  it('builds centered snippets with ellipses when the source is long', () => {
    const snippet = buildWorkspaceSearchSnippet(
      'Alpha planning notes with several detailed sections before the roadmap checkpoint and more trailing context afterwards.',
      { start: 64, end: 72 },
      32,
    );

    expect(snippet.text.startsWith('...')).toBe(true);
    expect(snippet.text.endsWith('...')).toBe(true);
    expect(snippet.highlightEnd).toBeGreaterThan(snippet.highlightStart);
  });
});
