import { describe, expect, it, vi } from 'vitest';
import { useEmbyHistoryStore } from '../../stores/embyHistoryStore';
import type { TranscriptSegment } from '../../types/transcript';
import {
  cleanSubtitleText,
  formatSrtTimestamp,
  generateBilingualSrt,
  generateOriginalSrt,
  generatePureTranslationSrt,
  syncEmbySubtitles,
} from '../emby/embyService';
import { TauriCommand } from '../tauri/commands';
import * as tauriInvoke from '../tauri/invoke';

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

  it('handles null, non-string, Windows newlines, and entity edge cases in cleanSubtitleText', () => {
    expect(cleanSubtitleText(null as unknown as string)).toBe('');
    expect(cleanSubtitleText(undefined as unknown as string)).toBe('');
    expect(cleanSubtitleText(123 as unknown as string)).toBe('');
    expect(cleanSubtitleText('Line 1\r\nLine 2\rLine 3')).toBe('Line 1\nLine 2\nLine 3');
    expect(cleanSubtitleText('It&#39;s &apos;good&#x27;')).toBe("It's 'good'");
    expect(cleanSubtitleText('Line 1\n\n\n\nLine 2')).toBe('Line 1\nLine 2');
  });

  it('enforces positive minimum cue duration in buildSrt', () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 5.0, end: 5.0, text: 'Instant cue', isFinal: true },
      { id: '2', start: 10.0, end: 8.0, text: 'Inverted cue', isFinal: true },
    ];
    const srt = generateOriginalSrt(segments);
    expect(srt).toContain('00:00:05,000 --> 00:00:05,050\nInstant cue');
    expect(srt).toContain('00:00:10,000 --> 00:00:10,050\nInverted cue');
  });

  it('deduplicates identical translation and original text in generateBilingualSrt', () => {
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 2, text: 'Hello', translation: 'hello', isFinal: true },
      { id: '2', start: 3, end: 5, text: 'World', translation: '世界', isFinal: true },
    ];
    const srt = generateBilingualSrt(segments);
    expect(srt).toContain('1\n00:00:00,000 --> 00:00:02,000\nhello');
    expect(srt).not.toContain('hello\nHello');
    expect(srt).toContain('2\n00:00:03,000 --> 00:00:05,000\n世界\nWorld');
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

describe('syncEmbySubtitles workflow', () => {
  it('routes pure_translation, bilingual, and separate_tracks correctly to invokeTauri', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue(undefined as never);

    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 2, text: 'Original line', translation: '翻译文本', isFinal: true },
    ];

    // 1. pure_translation
    await syncEmbySubtitles(segments, {
      serverId: 's1',
      serverUrl: 'http://localhost:8096',
      itemId: 'item-1',
      itemName: 'Test',
      audioStreamIndex: 1,
      subtitleMode: 'pure_translation',
      targetLanguage: 'zh-CN',
      existingExternalSubtitleIndex: 4,
    });

    expect(invokeSpy).toHaveBeenLastCalledWith(TauriCommand.emby.uploadSubtitle, {
      payload: {
        itemId: 'item-1',
        language: 'zh-CN',
        format: 'srt',
        isForced: false,
        content: expect.stringContaining('翻译文本'),
      },
      deleteExistingExternalIndex: 4,
    });

    // 2. separate_tracks
    invokeSpy.mockClear();
    await syncEmbySubtitles(segments, {
      serverId: 's1',
      serverUrl: 'http://localhost:8096',
      itemId: 'item-2',
      itemName: 'Test 2',
      audioStreamIndex: 1,
      subtitleMode: 'separate_tracks',
      sourceLanguage: 'en',
      targetLanguage: 'zh-CN',
      existingExternalSubtitleIndex: 7,
    });

    expect(invokeSpy).toHaveBeenCalledTimes(2);
    // First call: origContent (sourceLanguage) WITHOUT deleteExistingExternalIndex
    expect(invokeSpy).toHaveBeenNthCalledWith(1, TauriCommand.emby.uploadSubtitle, {
      payload: {
        itemId: 'item-2',
        language: 'en',
        format: 'srt',
        isForced: false,
        content: expect.stringContaining('Original line'),
      },
    });
    // Second call: transContent (targetLanguage) WITH deleteExistingExternalIndex: 7
    expect(invokeSpy).toHaveBeenNthCalledWith(2, TauriCommand.emby.uploadSubtitle, {
      payload: {
        itemId: 'item-2',
        language: 'zh-CN',
        format: 'srt',
        isForced: false,
        content: expect.stringContaining('翻译文本'),
      },
      deleteExistingExternalIndex: 7,
    });

    invokeSpy.mockRestore();
  });
});
