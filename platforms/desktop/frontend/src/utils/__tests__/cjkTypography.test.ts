import { describe, expect, it } from 'vitest';
import { formatCjkTypography } from '../cjkTypography';

describe('formatCjkTypography', () => {
  it('handles empty or blank string', () => {
    expect(formatCjkTypography('')).toBe('');
    expect(formatCjkTypography('   ')).toBe('   ');
  });

  it('inserts space between CJK and Latin characters', () => {
    expect(formatCjkTypography('识别到PR请求')).toBe('识别到 PR 请求');
    expect(formatCjkTypography('完成TODO事项')).toBe('完成 TODO 事项');
    expect(formatCjkTypography('hello世界')).toBe('hello 世界');
    expect(formatCjkTypography('世界hello')).toBe('世界 hello');
  });

  it('inserts space between CJK and Numbers', () => {
    expect(formatCjkTypography('第1阶段完成')).toBe('第 1 阶段完成');
    expect(formatCjkTypography('有99个人')).toBe('有 99 个人');
    expect(formatCjkTypography('今天是2026年9月19日')).toBe('今天是 2026 年 9 月 19 日');
  });

  it('normalizes ASCII punctuations adjacent to CJK into full-width', () => {
    expect(formatCjkTypography('你好,我是Sona')).toBe('你好，我是 Sona');
    expect(formatCjkTypography('你好,我是世界.')).toBe('你好，我是世界。');
    expect(formatCjkTypography('真的吗?当然!')).toBe('真的吗？当然！');
    expect(formatCjkTypography('请注意:重要通知')).toBe('请注意：重要通知');
  });

  it('removes spaces around CJK punctuations', () => {
    expect(formatCjkTypography('你好 ， 很好 。')).toBe('你好，很好。');
    expect(formatCjkTypography('【 重点 】 内容')).toBe('【重点】内容');
  });

  it('deduplicates duplicate Chinese punctuations', () => {
    expect(formatCjkTypography('好的。。没问题！！')).toBe('好的。没问题！');
    expect(formatCjkTypography('你好，，世界')).toBe('你好，世界');
  });

  it('preserves code snippets, URLs, and decimal numbers', () => {
    expect(formatCjkTypography('圆周率是3.14左右')).toBe('圆周率是 3.14 左右');
    expect(formatCjkTypography('时间是12:30分')).toBe('时间是 12:30 分');
    expect(formatCjkTypography('访问https://sona.ai即可')).toBe('访问 https://sona.ai 即可');
  });
});
