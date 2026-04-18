import { TextReplacementRule } from '../types/config';

/**
 * Applies enabled text replacement rules to a given text string.
 * Rules are applied sequentially in the order they appear in the array.
 * 
 * @param text The input text to process.
 * @param rules Array of text replacement rules.
 * @returns The processed text with replacements applied.
 */
export function applyTextReplacements(text: string, rules: TextReplacementRule[] | undefined): string {
    if (!text || !rules || rules.length === 0) {
        return text;
    }

    let result = text;
    
    // Sort rules: longer 'from' strings first to prevent partial matches 
    // (e.g. if we have "apple" -> "orange" and "apples" -> "oranges", 
    // we want "apples" to be processed first).
    const sortedRules = [...rules]
        .filter(rule => rule.enabled && rule.from)
        .sort((a, b) => b.from.length - a.from.length);

    for (const rule of sortedRules) {
        if (!rule.from) continue;
        
        // Use a global regular expression with proper escaping to replace all occurrences.
        // We escape special regex characters in the 'from' string.
        const escapedFrom = rule.from.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const flags = rule.ignoreCase ? 'gi' : 'g';
        const regex = new RegExp(escapedFrom, flags);
        result = result.replace(regex, rule.to || '');
    }

    return result;
}
