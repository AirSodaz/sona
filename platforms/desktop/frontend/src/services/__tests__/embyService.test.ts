import { describe, expect, it } from 'vitest';
import { useEmbyHistoryStore } from '../../stores/embyHistoryStore';
import type { TranscriptSegment } from '../../types/transcript';
import {
  cleanSubtitleText,
  formatSrtTimestamp,
  generateBilingualSrt,
  generateOriginalSrt,
  generatePureTranslationSrt,
} from '../emby/embyService';

describe('embyService SRT & text helpers', () => {
  it('formats SRT timestamp accurately without float truncation', () => {
    expect(formatSrtTimestamp(0)).toBe('00:00:00,000');
    expect(formatSrtTimestamp(1.005)).toBe('00:00:01,005');
    expect(formatSrtTimestamp(65.432)).toBe('00:01:05,432');
    expect(formatSrtTimestamp(3661.123)).toBe('01:01:01,123');
    expect(formatSrtTimestamp(-5)).toBe('00:00:00,000');
  });

  it('cleans subtitle text, handles HTML breaks, and prevents double-decoding', () => {
    expect(cleanSubtitleText('Line 1<br>Line 2<br />Line 3')).toBe('Line 1\nLine 2\nLine 3');
    expect(cleanSubtitleText('<p>Paragraph 1</p><p>Paragraph 2</p>')).toBe(
      'Paragraph 1\nParagraph 2'
    );
    // &amp;lt; should decode to &lt; and NOT double-decode to <
    expect(cleanSubtitleText('&amp;lt;div&amp;gt;')).toBe('&lt;div&gt;');
    expect(cleanSubtitleText('<b>bold &amp; proud</b>')).toBe('bold & proud');
    expect(cleanSubtitleText('&quot;Quote&#39;s&quot; &nbsp; space')).toBe('"Quote\'s"   space');
  });

  it('builds SRT, filters unfinalized segments, and adds trailing newline', () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 1.5, text: 'Hello', translation: '你好', isFinal: true },
      { id: '2', start: 1.5, end: 2.5, text: 'Drafting...', isFinal: false },
      { id: '3', start: 3.0, end: 4.2, text: 'World', translation: '世界', isFinal: true },
    ];

    const srtPure = generatePureTranslationSrt(segments);
    expect(srtPure).toContain('1\n00:00:00,000 --> 00:00:01,500\n你好');
    expect(srtPure).not.toContain('Drafting...');
    expect(srtPure).toContain('2\n00:00:03,000 --> 00:00:04,200\n世界');
    expect(srtPure.endsWith('\n')).toBe(true);

    const srtBilingual = generateBilingualSrt(segments);
    expect(srtBilingual).toContain('你好\nHello');
    expect(srtBilingual.endsWith('\n')).toBe(true);

    const srtOriginal = generateOriginalSrt(segments);
    expect(srtOriginal).toContain('Hello');
    expect(srtOriginal.endsWith('\n')).toBe(true);
  });
});

describe('useEmbyHistoryStore storage cap', () => {
  it('caps stored contexts to maximum 200 items (FIFO eviction)', () => {
    const store = useEmbyHistoryStore.getState();

    // Populate 205 items
    for (let i = 0; i < 205; i++) {
      store.setContext(`hist-${i}`, {
        serverId: 's1',
        serverUrl: 'http://localhost:8096',
        itemId: `item-${i}`,
        itemName: `Item ${i}`,
        audioStreamIndex: 1,
        subtitleMode: 'pure_translation',
        targetLanguage: 'zh-CN',
      });
    }

    const contexts = useEmbyHistoryStore.getState().contexts;
    const keys = Object.keys(contexts);
    expect(keys.length).toBe(200);

    // Oldest 5 items should have been evicted
    expect(store.getContext('hist-0')).toBeUndefined();
    expect(store.getContext('hist-4')).toBeUndefined();
    expect(store.getContext('hist-5')).toBeDefined();
    expect(store.getContext('hist-204')).toBeDefined();
  });
});
