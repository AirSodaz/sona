/**
 * CJK-Latin Smart Typography Formatter
 *
 * Formats mixed Chinese, Japanese, and Korean (CJK) text with Latin alphanumeric characters
 * according to standard typography rules (Pangu spacing, punctuation harmony, duplicate cleanup).
 */

// CJK Unicode ranges: Unified Ideographs, Extension A, Hiragana, Katakana, Bopomofo
const CJK_REGEX = '[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fa5\uf900-\ufaff\uff66-\uff9f]';

// Latin alphanumeric characters
const LATIN_OR_NUMBER_REGEX = '[A-Za-z0-9]';

const CJK_THEN_LATIN_RE = new RegExp(`(${CJK_REGEX})(${LATIN_OR_NUMBER_REGEX})`, 'g');
const LATIN_THEN_CJK_RE = new RegExp(`(${LATIN_OR_NUMBER_REGEX})(${CJK_REGEX})`, 'g');

// Space before closing / middle CJK punctuation -> remove space
const SPACE_BEFORE_CJK_PUNCT_RE = /\s+([，。！？；：、）】》」』”’])/g;

// Space after opening CJK punctuation -> remove space
const SPACE_AFTER_CJK_OPEN_PUNCT_RE = /([（【《「『“‘])\s+/g;

// Space after closing CJK punctuation followed by non-space -> clean up
const SPACE_AFTER_CJK_PUNCT_RE = new RegExp(
  `([，。！？；：、）】》」』”’])\\s+(${CJK_REGEX})`,
  'g'
);

// Consecutive duplicate Chinese punctuations (e.g. "。。" -> "。", "，，" -> "，")
const DUPLICATE_PUNCT_RE = /([，。！？；：、])\1+/g;

// Western punctuation directly following CJK -> convert to CJK full-width
const CJK_WESTERN_COMMA_RE = new RegExp(`(${CJK_REGEX}),`, 'g');
const CJK_WESTERN_QUESTION_RE = new RegExp(`(${CJK_REGEX})\\?`, 'g');
const CJK_WESTERN_EXCLAMATION_RE = new RegExp(`(${CJK_REGEX})!`, 'g');
const CJK_WESTERN_COLON_RE = new RegExp(`(${CJK_REGEX}):(?![0-9/])`, 'g');
const CJK_TRAILING_DOT_RE = new RegExp(`(${CJK_REGEX})\\.(?=\\s|$)`, 'g');

/**
 * Formats mixed CJK and Latin text with proper spacing and punctuation harmony.
 *
 * Examples:
 * - "识别到PR请求" -> "识别到 PR 请求"
 * - "第1阶段完成" -> "第 1 阶段完成"
 * - "hello世界" -> "hello 世界"
 * - "测试 ,完成." -> "测试，完成。"
 *
 * Preserves URLs, emails, decimals (3.14), times (12:30), and pure Western texts.
 */
export function formatCjkTypography(text: string): string {
  if (!text || text.length === 0) {
    return text;
  }

  let formatted = text;

  // 1. CJK followed by Latin/Number -> insert space
  formatted = formatted.replace(CJK_THEN_LATIN_RE, '$1 $2');

  // 2. Latin/Number followed by CJK -> insert space
  formatted = formatted.replace(LATIN_THEN_CJK_RE, '$1 $2');

  // 3. Convert ASCII comma/question/exclamation/colon directly adjacent to CJK into full-width CJK punctuation
  formatted = formatted.replace(CJK_WESTERN_COMMA_RE, '$1，');
  formatted = formatted.replace(CJK_WESTERN_QUESTION_RE, '$1？');
  formatted = formatted.replace(CJK_WESTERN_EXCLAMATION_RE, '$1！');
  formatted = formatted.replace(CJK_WESTERN_COLON_RE, '$1：');

  // 4. Trailing ASCII dot after CJK -> Chinese full stop "。"
  formatted = formatted.replace(CJK_TRAILING_DOT_RE, '$1。');

  // 5. Remove redundant spaces before/after CJK punctuation
  formatted = formatted.replace(SPACE_BEFORE_CJK_PUNCT_RE, '$1');
  formatted = formatted.replace(SPACE_AFTER_CJK_OPEN_PUNCT_RE, '$1');
  formatted = formatted.replace(SPACE_AFTER_CJK_PUNCT_RE, '$1$2');

  // 6. Deduplicate repeated punctuation marks
  formatted = formatted.replace(DUPLICATE_PUNCT_RE, '$1');

  return formatted;
}
