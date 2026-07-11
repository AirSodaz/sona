export interface WorkspaceSearchRange {
  start: number;
  end: number;
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

export function getWorkspaceSearchResultDomId(id: string): string {
  return `workspace-search-result-${id}`;
}
