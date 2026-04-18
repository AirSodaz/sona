import { describe, it, expect } from 'vitest';
import { applyTextReplacements } from '../textProcessing';
import { TextReplacementRuleSet } from '../../types/config';

describe('applyTextReplacements', () => {
    it('returns original text if no sets are provided', () => {
        expect(applyTextReplacements('hello world', undefined)).toBe('hello world');
        expect(applyTextReplacements('hello world', [])).toBe('hello world');
    });

    it('returns original text if no sets are enabled', () => {
        const sets: TextReplacementRuleSet[] = [
            {
                id: '1',
                name: 'Test Set',
                enabled: false,
                ignoreCase: false,
                rules: [{ id: 'r1', from: 'apple', to: 'orange' }]
            }
        ];
        expect(applyTextReplacements('I like apple', sets)).toBe('I like apple');
    });

    it('applies basic replacements from an enabled set', () => {
        const sets: TextReplacementRuleSet[] = [
            {
                id: '1',
                name: 'Test Set',
                enabled: true,
                ignoreCase: false,
                rules: [{ id: 'r1', from: 'apple', to: 'orange' }]
            }
        ];
        expect(applyTextReplacements('I like apple', sets)).toBe('I like orange');
    });

    it('supports case-insensitive matching at the set level', () => {
        const sets: TextReplacementRuleSet[] = [
            {
                id: '1',
                name: 'Test Set',
                enabled: true,
                ignoreCase: true,
                rules: [{ id: 'r1', from: 'APPLE', to: 'orange' }]
            }
        ];
        expect(applyTextReplacements('I like apple', sets)).toBe('I like orange');
    });

    it('respects case-sensitivity by default', () => {
        const sets: TextReplacementRuleSet[] = [
            {
                id: '1',
                name: 'Test Set',
                enabled: true,
                ignoreCase: false,
                rules: [{ id: 'r1', from: 'APPLE', to: 'orange' }]
            }
        ];
        expect(applyTextReplacements('I like apple', sets)).toBe('I like apple');
    });

    it('applies multiple sets', () => {
        const sets: TextReplacementRuleSet[] = [
            {
                id: '1',
                name: 'Set 1',
                enabled: true,
                ignoreCase: false,
                rules: [{ id: 'r1', from: 'apple', to: 'orange' }]
            },
            {
                id: '2',
                name: 'Set 2',
                enabled: true,
                ignoreCase: false,
                rules: [{ id: 'r2', from: 'banana', to: 'grape' }]
            }
        ];
        expect(applyTextReplacements('apple and banana', sets)).toBe('orange and grape');
    });

    it('applies longer rules first even across different sets', () => {
        const sets: TextReplacementRuleSet[] = [
            {
                id: '1',
                name: 'Set 1',
                enabled: true,
                ignoreCase: false,
                rules: [{ id: 'r1', from: 'apple', to: 'orange' }]
            },
            {
                id: '2',
                name: 'Set 2',
                enabled: true,
                ignoreCase: false,
                rules: [{ id: 'r2', from: 'apples', to: 'oranges' }]
            }
        ];
        expect(applyTextReplacements('I have many apples', sets)).toBe('I have many oranges');
    });

    it('handles regex special characters', () => {
        const sets: TextReplacementRuleSet[] = [
            {
                id: '1',
                name: 'Set 1',
                enabled: true,
                ignoreCase: false,
                rules: [{ id: 'r1', from: 'price $5.00?', to: 'free' }]
            }
        ];
        expect(applyTextReplacements('is it price $5.00?', sets)).toBe('is it free');
    });
});
