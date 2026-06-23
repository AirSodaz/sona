/**
 * Converts old format (`<b>/<i>/<u>`) to Lexical format (`<strong>/<em>/<u>`)
 * and ensures content is wrapped in `<p>` tags.
 *
 * Old format example: "Hello <b>World</b>"
 * New format example: "<p>Hello <strong>World</strong></p>"
 *
 * If the text is already in Lexical format (contains `<p>` wrapper),
 * it's returned unchanged.
 */
export function convertOldFormatToLexical(text: string): string {
  if (!text) return '';

  // Detect Lexical format: starts with a block tag or inline formatting tag
  if (text.startsWith('<p>') || text.startsWith('<strong>') || text.startsWith('<em>') || text.startsWith('<u>')) {
    return text;
  }

  let result = text
    .replace(/<b>/g, '<strong>')
    .replace(/<\/b>/g, '</strong>')
    .replace(/<i>/g, '<em>')
    .replace(/<\/i>/g, '</em>');

  // Wrap in <p> to match Lexical's paragraph block structure
  if (!result.startsWith('<p>')) {
    result = `<p>${result}</p>`;
  }

  return result;
}
