export interface WorkspaceSearchRange {
  start: number;
  end: number;
}

interface NormalizedCharSegment {
  start: number;
  end: number;
}

export interface NormalizedWorkspaceSearchText {
  text: string;
  rawSegments: NormalizedCharSegment[];
}

export interface WorkspaceSearchSnippet {
  text: string;
  highlightStart: number;
  highlightEnd: number;
}

export interface WorkspaceItemSearchMatch {
  matchedField: 'title' | 'previewText' | 'searchContent';
  titleMatch: WorkspaceSearchRange | null;
  displaySnippet: WorkspaceSearchSnippet;
}

export interface WorkspaceSearchableItem {
  title: string;
  previewText?: string;
  searchContent?: string;
}

const DEFAULT_SNIPPET_LENGTH = 72;

const PUNCTUATION_MAP: Record<string, string> = {
  '，': ',',
  '、': ',',
  '。': '.',
  '．': '.',
  '：': ':',
  '；': ';',
  '！': '!',
  '？': '?',
  '（': '(',
  '）': ')',
  '【': '[',
  '】': ']',
  '［': '[',
  '］': ']',
  '｛': '{',
  '｝': '}',
  '《': '<',
  '》': '>',
  '〈': '<',
  '〉': '>',
  '“': '"',
  '”': '"',
  '‘': '\'',
  '’': '\'',
  '「': '"',
  '」': '"',
  '『': '"',
  '』': '"',
  '—': '-',
  '–': '-',
  '－': '-',
  '〜': '~',
  '～': '~',
  '／': '/',
  '＼': '\\',
  '｜': '|',
  '·': '.',
  '・': '.',
};

function normalizeSearchChunk(value: string): string {
  return Array.from(value.normalize('NFKC').toLowerCase())
    .map((char) => PUNCTUATION_MAP[char] ?? char)
    .join('');
}

export function normalizeWorkspaceSearchText(value: string): NormalizedWorkspaceSearchText {
  const normalizedChars: string[] = [];
  const rawSegments: NormalizedCharSegment[] = [];

  let rawOffset = 0;
  for (const char of value) {
    const rawStart = rawOffset;
    rawOffset += char.length;
    const rawEnd = rawOffset;

    const normalizedChunk = normalizeSearchChunk(char);
    if (!normalizedChunk) {
      continue;
    }

    for (const normalizedChar of Array.from(normalizedChunk)) {
      if (/\s/u.test(normalizedChar)) {
        if (normalizedChars.length === 0) {
          continue;
        }

        if (normalizedChars[normalizedChars.length - 1] === ' ') {
          rawSegments[rawSegments.length - 1].end = rawEnd;
          continue;
        }

        normalizedChars.push(' ');
        rawSegments.push({ start: rawStart, end: rawEnd });
        continue;
      }

      normalizedChars.push(normalizedChar);
      rawSegments.push({ start: rawStart, end: rawEnd });
    }
  }

  if (normalizedChars[normalizedChars.length - 1] === ' ') {
    normalizedChars.pop();
    rawSegments.pop();
  }

  return {
    text: normalizedChars.join(''),
    rawSegments,
  };
}

export function findWorkspaceSearchMatchRange(value: string, normalizedQuery: string): WorkspaceSearchRange | null {
  if (!normalizedQuery) {
    return null;
  }

  const normalized = normalizeWorkspaceSearchText(value);
  const normalizedMatchStart = normalized.text.indexOf(normalizedQuery);
  if (normalizedMatchStart === -1) {
    return null;
  }

  const normalizedMatchEnd = normalizedMatchStart + normalizedQuery.length - 1;
  return {
    start: normalized.rawSegments[normalizedMatchStart].start,
    end: normalized.rawSegments[normalizedMatchEnd].end,
  };
}

export function buildWorkspaceSearchSnippet(
  value: string,
  range: WorkspaceSearchRange,
  maxLength = DEFAULT_SNIPPET_LENGTH,
): WorkspaceSearchSnippet {
  const safeStart = Math.max(0, Math.min(range.start, value.length));
  const safeEnd = Math.max(safeStart + 1, Math.min(range.end, value.length));
  const matchLength = safeEnd - safeStart;
  const snippetLength = Math.max(maxLength, matchLength);
  const contextLength = Math.max(0, Math.floor((snippetLength - matchLength) / 2));

  let sliceStart = Math.max(0, safeStart - contextLength);
  let sliceEnd = Math.min(value.length, safeEnd + contextLength);

  if (sliceEnd - sliceStart < snippetLength) {
    if (sliceStart === 0) {
      sliceEnd = Math.min(value.length, snippetLength);
    } else if (sliceEnd === value.length) {
      sliceStart = Math.max(0, value.length - snippetLength);
    }
  }

  let snippetText = value.slice(sliceStart, sliceEnd);
  const leadingWhitespace = snippetText.match(/^\s+/u)?.[0].length ?? 0;
  const trailingWhitespace = snippetText.match(/\s+$/u)?.[0].length ?? 0;

  if (leadingWhitespace > 0 || trailingWhitespace > 0) {
    snippetText = snippetText.slice(
      leadingWhitespace,
      trailingWhitespace > 0 ? snippetText.length - trailingWhitespace : undefined,
    );
    sliceStart += leadingWhitespace;
    sliceEnd -= trailingWhitespace;
  }

  let highlightStart = safeStart - sliceStart;
  let highlightEnd = safeEnd - sliceStart;
  let prefix = '';
  let suffix = '';

  if (sliceStart > 0) {
    prefix = '...';
    highlightStart += prefix.length;
    highlightEnd += prefix.length;
  }

  if (sliceEnd < value.length) {
    suffix = '...';
  }

  return {
    text: `${prefix}${snippetText}${suffix}`,
    highlightStart,
    highlightEnd,
  };
}

export function matchWorkspaceItem(item: WorkspaceSearchableItem, query: string): WorkspaceItemSearchMatch | null {
  const normalizedQuery = normalizeWorkspaceSearchText(query).text;
  if (!normalizedQuery) {
    return null;
  }

  const titleMatch = findWorkspaceSearchMatchRange(item.title, normalizedQuery);
  const previewText = item.previewText ?? '';
  const previewMatch = previewText
    ? findWorkspaceSearchMatchRange(previewText, normalizedQuery)
    : null;
  const searchContent = item.searchContent ?? '';
  const searchContentMatch = searchContent
    ? findWorkspaceSearchMatchRange(searchContent, normalizedQuery)
    : null;

  if (!titleMatch && !previewMatch && !searchContentMatch) {
    return null;
  }

  const matchedField = titleMatch
    ? 'title'
    : previewMatch
    ? 'previewText'
    : 'searchContent';

  const displaySourceText = previewMatch
    ? previewText
    : searchContentMatch
    ? searchContent
    : item.title;
  const displayRange = previewMatch ?? searchContentMatch ?? titleMatch;

  return {
    matchedField,
    titleMatch,
    displaySnippet: buildWorkspaceSearchSnippet(displaySourceText, displayRange!),
  };
}

export function getWorkspaceSearchResultDomId(id: string): string {
  return `workspace-search-result-${id}`;
}
