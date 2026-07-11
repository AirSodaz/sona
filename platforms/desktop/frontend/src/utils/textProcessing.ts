import { TextReplacementRuleSet } from '../types/config';

/**
 * Applies enabled text replacement rules from multiple rule sets to a given text string.
 * Rules are gathered from all enabled sets and applied sequentially.
 * Within each set, longer 'from' strings are applied first.
 * 
 * @param text The input text to process.
 * @param sets Array of text replacement rule sets.
 * @returns The processed text with replacements applied.
 */
export function applyTextReplacements(text: string, sets: TextReplacementRuleSet[] | undefined): string {
    if (!text || !sets || sets.length === 0) {
        return text;
    }

    let result = text;
    
    // Gather all rules from enabled sets
    const enabledSets = sets.filter(set => set.enabled && set.rules.length > 0);
    if (enabledSets.length === 0) {
        return text;
    }

    // To prevent partial matches across different sets, we flatten and sort all rules by length
    const allActiveRules = enabledSets.flatMap(set => 
        set.rules
            .filter(rule => rule.from)
            .map(rule => ({
                from: rule.from,
                to: rule.to,
                ignoreCase: set.ignoreCase
            }))
    ).sort((a, b) => b.from.length - a.from.length);

    for (const rule of allActiveRules) {
        // Use a global regular expression with proper escaping to replace all occurrences.
        const escapedFrom = rule.from.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const flags = rule.ignoreCase ? 'gi' : 'g';
        const regex = new RegExp(escapedFrom, flags);
        result = result.replace(regex, rule.to || '');
    }

    return result;
}

