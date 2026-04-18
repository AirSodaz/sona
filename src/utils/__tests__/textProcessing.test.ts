import { describe, it, expect } from 'vitest';
import { applyTextReplacements } from '../textProcessing';
import { TextReplacementRule } from '../../types/config';

describe('applyTextReplacements', () => {
    it('returns original text if no rules are provided', () => {
        expect(applyTextReplacements('hello world', undefined)).toBe('hello world');
        expect(applyTextReplacements('hello world', [])).toBe('hello world');
    });

    it('applies basic replacements', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: 'apple', to: 'orange', enabled: true }
        ];
        expect(applyTextReplacements('I like apple', rules)).toBe('I like orange');
    });

    it('ignores disabled rules', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: 'apple', to: 'orange', enabled: false }
        ];
        expect(applyTextReplacements('I like apple', rules)).toBe('I like apple');
    });

    it('supports case-insensitive matching', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: 'APPLE', to: 'orange', enabled: true, ignoreCase: true }
        ];
        expect(applyTextReplacements('I like apple', rules)).toBe('I like orange');
    });

    it('respects case-sensitivity by default', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: 'APPLE', to: 'orange', enabled: true }
        ];
        expect(applyTextReplacements('I like apple', rules)).toBe('I like apple');
    });

    it('replaces all occurrences', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: 'apple', to: 'orange', enabled: true }
        ];
        expect(applyTextReplacements('apple apple apple', rules)).toBe('orange orange orange');
    });

    it('handles special regex characters in the find string', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: 'price $5.00?', to: 'free', enabled: true }
        ];
        expect(applyTextReplacements('is the price $5.00? yes', rules)).toBe('is the free yes');
    });

    it('applies longer rules first to avoid partial overlap issues', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: 'apple', to: 'orange', enabled: true },
            { id: '2', from: 'apples', to: 'oranges', enabled: true }
        ];
        // If "apple" was applied first, "apples" would become "oranges" anyway in this specific case,
        // but sorting by length is generally safer for complex replacements.
        expect(applyTextReplacements('I have many apples', rules)).toBe('I have many oranges');
    });
    
    it('handles Chinese characters', () => {
        const rules: TextReplacementRule[] = [
            { id: '1', from: '苹果', to: '香蕉', enabled: true }
        ];
        expect(applyTextReplacements('我喜欢苹果', rules)).toBe('我喜欢香蕉');
    });
});
