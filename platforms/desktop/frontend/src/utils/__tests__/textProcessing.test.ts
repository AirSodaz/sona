import { describe, expect, it } from 'vitest';
import type { TextReplacementRuleSet } from '../../types/config';
import { applyTextReplacements, expandTextMacros } from '../textProcessing';

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
        rules: [{ id: 'r1', from: 'apple', to: 'orange' }],
      },
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
        rules: [{ id: 'r1', from: 'apple', to: 'orange' }],
      },
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
        rules: [{ id: 'r1', from: 'APPLE', to: 'orange' }],
      },
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
        rules: [{ id: 'r1', from: 'APPLE', to: 'orange' }],
      },
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
        rules: [{ id: 'r1', from: 'apple', to: 'orange' }],
      },
      {
        id: '2',
        name: 'Set 2',
        enabled: true,
        ignoreCase: false,
        rules: [{ id: 'r2', from: 'banana', to: 'grape' }],
      },
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
        rules: [{ id: 'r1', from: 'apple', to: 'orange' }],
      },
      {
        id: '2',
        name: 'Set 2',
        enabled: true,
        ignoreCase: false,
        rules: [{ id: 'r2', from: 'apples', to: 'oranges' }],
      },
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
        rules: [{ id: 'r1', from: 'price $5.00?', to: 'free' }],
      },
    ];
    expect(applyTextReplacements('is it price $5.00?', sets)).toBe('is it free');
  });
  it('expands dynamic macros like {today} and {time}', () => {
    const fixedDate = new Date(2026, 8, 19, 14, 30, 0); // 2026-09-19 14:30:00
    expect(expandTextMacros('今天是 {today}', fixedDate)).toBe('今天是 2026-09-19');
    expect(expandTextMacros('现在时间 {time}', fixedDate)).toBe('现在时间 14:30:00');

    const sets: TextReplacementRuleSet[] = [
      {
        id: 'macros',
        name: 'Macros',
        enabled: true,
        ignoreCase: true,
        rules: [
          { id: 'm1', from: '我的邮箱', to: 'asoda@outlook.com' },
          { id: 'm2', from: '当前日期', to: '{date}' },
        ],
      },
    ];

    expect(applyTextReplacements('请发送到 我的邮箱 谢谢', sets)).toBe(
      '请发送到 asoda@outlook.com 谢谢'
    );
  });
});
