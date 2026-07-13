export type EditorShortcutKey =
  | 'cut'
  | 'copy'
  | 'paste'
  | 'selectAll'
  | 'bold'
  | 'italic'
  | 'underline';

const SHORTCUT_KEYS: Record<EditorShortcutKey, string> = {
  cut: 'X',
  copy: 'C',
  paste: 'V',
  selectAll: 'A',
  bold: 'B',
  italic: 'I',
  underline: 'U',
};

export function getEditorShortcut(key: EditorShortcutKey): string {
  const platform = typeof navigator === 'undefined' ? '' : navigator.platform;
  const modifier = /Mac|iPhone|iPad|iPod/i.test(platform) ? 'Cmd' : 'Ctrl';
  return `${modifier}+${SHORTCUT_KEYS[key]}`;
}
