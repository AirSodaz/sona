import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useEmbyHistoryStore } from '../../stores/embyHistoryStore';
import type { TranscriptSegment } from '../../types/transcript';
import {
  cleanSubtitleText,
  downloadEmbyAudio,
  formatSrtTimestamp,
  generateBilingualSrt,
  generateOriginalSrt,
  generatePureTranslationSrt,
  getEmbyConfig,
  getEmbyItems,
  getEmbyLibraries,
  getEmbyProcessingSettings,
  saveEmbyConfig,
  saveEmbyProcessingSettings,
  syncEmbySubtitles,
  testEmbyConnection,
  uploadEmbySubtitle,
} from '../emby/embyService';
import { TauriCommand } from '../tauri/commands';
import * as tauriInvoke from '../tauri/invoke';

beforeEach(() => {
  useEmbyHistoryStore.setState({ contexts: {} });
  localStorage.removeItem('sona:emby:history_contexts');
});

afterEach(() => {
  vi.restoreAllMocks();
  useEmbyHistoryStore.setState({ contexts: {} });
  localStorage.removeItem('sona:emby:history_contexts');
});
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
    expect(cleanSubtitleText('<<script>script>alert(1)</script>Clean text')).toBe(
      'alert(1)Clean text'
    );
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
    // First call: transContent (targetLanguage) WITH deleteExistingExternalIndex: 7
    expect(invokeSpy).toHaveBeenNthCalledWith(1, TauriCommand.emby.uploadSubtitle, {
      payload: {
        itemId: 'item-2',
        language: 'zh-CN',
        format: 'srt',
        isForced: false,
        content: expect.stringContaining('翻译文本'),
      },
      deleteExistingExternalIndex: 7,
    });
    // Second call: origContent (sourceLanguage) WITHOUT deleteExistingExternalIndex
    expect(invokeSpy).toHaveBeenNthCalledWith(2, TauriCommand.emby.uploadSubtitle, {
      payload: {
        itemId: 'item-2',
        language: 'en',
        format: 'srt',
        isForced: false,
        content: expect.stringContaining('Original line'),
      },
    });

    invokeSpy.mockRestore();
  });

  it('uploads combined text in bilingual mode', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue(undefined as never);
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 2, text: 'Hello world', translation: '你好世界', isFinal: true },
    ];

    await syncEmbySubtitles(
      segments,
      {
        serverId: 's1',
        serverUrl: 'http://localhost:8096',
        itemId: 'item-bilingual',
        itemName: 'Bilingual Movie',
        audioStreamIndex: 0,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
      },
      { deleteExistingIndex: 3 }
    );

    expect(invokeSpy).toHaveBeenCalledTimes(1);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.uploadSubtitle, {
      payload: {
        itemId: 'item-bilingual',
        language: 'zh-CN',
        format: 'srt',
        isForced: false,
        content: expect.stringContaining('你好世界\nHello world'),
      },
      deleteExistingExternalIndex: 3,
    });

    invokeSpy.mockRestore();
  });

  it('does not upload transContent when segments lack translation in separate_tracks mode', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue(undefined as never);
    const segments: TranscriptSegment[] = [
      { id: '1', start: 0, end: 2, text: 'Only original', isFinal: true },
    ];

    await syncEmbySubtitles(
      segments,
      {
        serverId: 's1',
        serverUrl: 'http://localhost:8096',
        itemId: 'item-no-trans',
        itemName: 'No Trans Movie',
        audioStreamIndex: 0,
        subtitleMode: 'separate_tracks',
        sourceLanguage: 'en',
        targetLanguage: 'zh-CN',
        existingExternalSubtitleIndex: 4,
      },
      { deleteExistingIndex: 4 }
    );

    expect(invokeSpy).toHaveBeenCalledTimes(1);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.uploadSubtitle, {
      payload: {
        itemId: 'item-no-trans',
        language: 'en',
        format: 'srt',
        isForced: false,
        content: expect.stringContaining('Only original'),
      },
      deleteExistingExternalIndex: undefined,
    });

    invokeSpy.mockRestore();
  });
});
describe('embyService IPC wrappers', () => {
  it('calls testEmbyConnection with expected command and payload', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue({
      id: 'server-1',
      serverName: 'Emby Server',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
    });
    const config = {
      serverUrl: 'http://localhost:8096',
      apiKey: 'key-1',
      allowInvalidCerts: false,
    };
    const res = await testEmbyConnection(config);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.testConnection, { config });
    expect(res.serverName).toBe('Emby Server');
  });

  it('calls saveEmbyConfig with expected command and payload', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue(undefined);
    const config = {
      serverUrl: 'http://localhost:8096',
      apiKey: 'key-1',
      allowInvalidCerts: false,
    };
    await saveEmbyConfig(config);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.saveConfig, { config });
  });

  it('calls getEmbyConfig with expected command', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue({
      serverUrl: 'http://localhost:8096',
      apiKey: 'key-1',
      allowInvalidCerts: false,
    });
    const res = await getEmbyConfig();
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.getConfig);
    expect(res?.serverUrl).toBe('http://localhost:8096');
  });

  it('calls saveEmbyProcessingSettings with expected command and payload', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue(undefined);
    const settings = {
      asrLanguage: 'zh',
      defaultSubtitleMode: 'bilingual' as const,
      overwriteExistingExternal: true,
    };
    await saveEmbyProcessingSettings(settings);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.saveProcessingSettings, { settings });
  });

  it('calls getEmbyProcessingSettings with expected command', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue({
      asrLanguage: 'ja',
      defaultSubtitleMode: 'pure_translation',
      overwriteExistingExternal: false,
    });
    const res = await getEmbyProcessingSettings();
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.getProcessingSettings);
    expect(res?.asrLanguage).toBe('ja');
    expect(res?.defaultSubtitleMode).toBe('pure_translation');
  });

  it('calls getEmbyLibraries with expected command', async () => {
    const invokeSpy = vi
      .spyOn(tauriInvoke, 'invokeTauri')
      .mockResolvedValue([{ id: 'lib-1', name: 'Movies' }]);
    const res = await getEmbyLibraries();
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.getLibraries);
    expect(res).toHaveLength(1);
    expect(res[0].name).toBe('Movies');
  });

  it('calls getEmbyItems with expected command and params', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue({
      items: [],
      totalRecordCount: 0,
    });
    const params = { parentId: 'lib-1', searchTerm: 'test', startIndex: 0, limit: 50 };
    const res = await getEmbyItems(params);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.getItems, params);
    expect(res.totalRecordCount).toBe(0);
  });

  it('calls downloadEmbyAudio with expected command and arguments', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue('/tmp/audio.mp3');
    const res = await downloadEmbyAudio('item-123', 1);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.downloadAudio, {
      itemId: 'item-123',
      streamIndex: 1,
    });
    expect(res).toBe('/tmp/audio.mp3');
  });

  it('calls uploadEmbySubtitle with expected command and arguments', async () => {
    const invokeSpy = vi.spyOn(tauriInvoke, 'invokeTauri').mockResolvedValue(undefined);
    const payload = {
      itemId: 'item-123',
      language: 'zh-CN',
      format: 'srt',
      isForced: false,
      content: '1\n00:00:00,000 --> 00:00:01,000\nHello\n',
    };
    await uploadEmbySubtitle(payload, 2);
    expect(invokeSpy).toHaveBeenCalledWith(TauriCommand.emby.uploadSubtitle, {
      payload,
      deleteExistingExternalIndex: 2,
    });
  });
});
