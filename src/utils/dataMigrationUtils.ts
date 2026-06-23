/**
 * Converts old format (`<b>/<i>/<u>`) to Lexical format (`<strong>/<em>/<u>`)
 * and ensures content is wrapped in `<p>` tags.
 *
 * Old format example: "Hello <b>World</b>"
 * New format example: "<p>Hello <strong>World</strong></p>"
 *
 * The function is idempotent: passing already-converted Lexical HTML
 * produces the same result.
 */
export function convertOldFormatToLexical(text: string): string {
  if (!text) return '';

  // Strip any existing <p> wrappers to avoid double-wrapping
  let result = text.replace(/<\/?p[^>]*>/gi, '');

  // Convert old format tags to Lexical equivalents
  result = result
    .replace(/<b>/gi, '<strong>')
    .replace(/<\/b>/gi, '</strong>')
    .replace(/<i>/gi, '<em>')
    .replace(/<\/i>/gi, '</em>');

  // Wrap in <p> to match Lexical's paragraph block structure
  return `<p>${result}</p>`;
}
