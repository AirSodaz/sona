import type { TextReplacementRuleSet } from '../types/config';

/**
 * Applies enabled text replacement rules from multiple rule sets to a given text string.
 * Rules are gathered from all enabled sets and applied sequentially.
 * Within each set, longer 'from' strings are applied first.
 *
 * @param text The input text to process.
 * @param sets Array of text replacement rule sets.
 * @returns The processed text with replacements applied.
 */
export function applyTextReplacements(
  text: string,
  sets: TextReplacementRuleSet[] | undefined
): string {
  if (!text || !sets || sets.length === 0) {
    return text;
  }

  let result = text;

  // Gather all rules from enabled sets
  const enabledSets = sets.filter((set) => set.enabled && set.rules.length > 0);
  if (enabledSets.length === 0) {
    return text;
  }

  // To prevent partial matches across different sets, we flatten and sort all rules by length
  const allActiveRules = enabledSets
    .flatMap((set) =>
      set.rules
        .filter((rule) => rule.from)
        .map((rule) => ({
          from: rule.from,
          to: rule.to,
          ignoreCase: set.ignoreCase,
        }))
    )
    .sort((a, b) => b.from.length - a.from.length);

  for (const rule of allActiveRules) {
    // Use a global regular expression with proper escaping to replace all occurrences.
    const escapedFrom = rule.from.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const flags = rule.ignoreCase ? 'gi' : 'g';
    const regex = new RegExp(escapedFrom, flags);
    const target = expandTextMacros(rule.to || '');
    result = result.replace(regex, target);
  }

  return result;
}
/**
 * Expands dynamic macro placeholders like {date}, {today}, {time}, {datetime}, {year}.
 */
export function expandTextMacros(text: string, now: Date = new Date()): string {
  if (!text?.includes('{')) {
    return text;
  }

  const pad = (n: number) => n.toString().padStart(2, '0');
  const year = now.getFullYear().toString();
  const month = pad(now.getMonth() + 1);
  const day = pad(now.getDate());
  const hours = pad(now.getHours());
  const minutes = pad(now.getMinutes());
  const seconds = pad(now.getSeconds());

  const dateStr = `${year}-${month}-${day}`;
  const timeStr = `${hours}:${minutes}:${seconds}`;
  const dateTimeStr = `${dateStr} ${timeStr}`;

  return text
    .replace(/\{(?:date|today)\}/gi, dateStr)
    .replace(/\{time\}/gi, timeStr)
    .replace(/\{datetime\}/gi, dateTimeStr)
    .replace(/\{year\}/gi, year)
    .replace(/\{month\}/gi, month)
    .replace(/\{day\}/gi, day);
}
