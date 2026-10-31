import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { MediaServerTaskContext } from '../../types/batchQueue';
import type { TranscriptSegment } from '../../types/transcript';
import {
  cleanSubtitleText,
  formatSrtTimestamp,
  generateBilingualSrt,
  generateOriginalSrt,
  generatePureTranslationSrt,
  syncMediaServerSubtitles,
} from '../mediaServer/mediaServerService';
import { TauriCommand } from '../tauri/commands';
import { invokeTauri } from '../tauri/invoke';

vi.mock('../tauri/invoke', () => ({
  invokeTauri: vi.fn(),
}));

describe('mediaServerService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('formatSrtTimestamp', () => {
    it('formats seconds into HH:MM:SS,mmm', () => {
      expect(formatSrtTimestamp(0)).toBe('00:00:00,000');
      expect(formatSrtTimestamp(1.5)).toBe('00:00:01,500');
      expect(formatSrtTimestamp(65.123)).toBe('00:01:05,123');
      expect(formatSrtTimestamp(3661.004)).toBe('01:01:01,004');
    });
  });

  describe('cleanSubtitleText', () => {
    it('strips html tags and decodes entities', () => {
      expect(cleanSubtitleText('<b>Hello</b> &amp; world')).toBe('Hello & world');
      expect(cleanSubtitleText('Line 1<br/>Line 2')).toBe('Line 1\nLine 2');
    });
  });

  describe('SRT Generators', () => {
    const sampleSegments: TranscriptSegment[] = [
      {
        id: '1',
        start: 1,
        end: 3,
        text: 'Hello world',
        translation: '你好世界',
        isFinal: true,
      },
      {
        id: '2',
        start: 4,
        end: 6,
        text: 'How are you?',
        translation: '你好吗？',
        isFinal: true,
      },
    ];

    it('generates pure translation SRT', () => {
      const srt = generatePureTranslationSrt(sampleSegments);
      expect(srt).toContain('1\n00:00:01,000 --> 00:00:03,000\n你好世界');
      expect(srt).toContain('2\n00:00:04,000 --> 00:00:06,000\n你好吗？');
    });

    it('generates original SRT', () => {
      const srt = generateOriginalSrt(sampleSegments);
      expect(srt).toContain('1\n00:00:01,000 --> 00:00:03,000\nHello world');
      expect(srt).toContain('2\n00:00:04,000 --> 00:00:06,000\nHow are you?');
    });

    it('generates bilingual SRT', () => {
      const srt = generateBilingualSrt(sampleSegments);
      expect(srt).toContain('1\n00:00:01,000 --> 00:00:03,000\n你好世界\nHello world');
      expect(srt).toContain('2\n00:00:04,000 --> 00:00:06,000\n你好吗？\nHow are you?');
    });
  });

  describe('syncMediaServerSubtitles', () => {
    const context: MediaServerTaskContext = {
      serverId: 'srv-jellyfin-99',
      serverType: 'jellyfin',
      serverUrl: 'http://localhost:8096',
      itemId: 'item-101',
      itemName: 'Test Movie',
      audioStreamIndex: 1,
      subtitleMode: 'bilingual',
      targetLanguage: 'zh-CN',
      existingExternalSubtitleIndex: 3,
    };

    const segments: TranscriptSegment[] = [
      {
        id: '1',
        start: 0,
        end: 2,
        text: 'Welcome',
        translation: '欢迎',
        isFinal: true,
      },
    ];

    it('syncs bilingual subtitles passing target serverId and delete index', async () => {
      vi.mocked(invokeTauri).mockResolvedValue(undefined);

      await syncMediaServerSubtitles(segments, context);

      expect(invokeTauri).toHaveBeenCalledWith(TauriCommand.mediaServer.uploadSubtitle, {
        serverId: 'srv-jellyfin-99',
        deleteExistingExternalIndex: 3,
        payload: {
          itemId: 'item-101',
          language: 'zh-CN',
          format: 'srt',
          isForced: false,
          content: expect.stringContaining('欢迎\nWelcome'),
        },
      });
    });

    it('syncs pure translation subtitles', async () => {
      vi.mocked(invokeTauri).mockResolvedValue(undefined);

      await syncMediaServerSubtitles(segments, {
        ...context,
        subtitleMode: 'pure_translation',
      });

      expect(invokeTauri).toHaveBeenCalledWith(TauriCommand.mediaServer.uploadSubtitle, {
        serverId: 'srv-jellyfin-99',
        deleteExistingExternalIndex: 3,
        payload: {
          itemId: 'item-101',
          language: 'zh-CN',
          format: 'srt',
          isForced: false,
          content: expect.stringContaining('欢迎'),
        },
      });
    });

    it('syncs separate tracks subtitles uploading both translation and original tracks', async () => {
      vi.mocked(invokeTauri).mockResolvedValue(undefined);

      await syncMediaServerSubtitles(segments, {
        ...context,
        subtitleMode: 'separate_tracks',
        sourceLanguage: 'en',
      });

      expect(invokeTauri).toHaveBeenCalledTimes(2);
      expect(invokeTauri).toHaveBeenNthCalledWith(1, TauriCommand.mediaServer.uploadSubtitle, {
        serverId: 'srv-jellyfin-99',
        deleteExistingExternalIndex: 3,
        payload: {
          itemId: 'item-101',
          language: 'zh-CN',
          format: 'srt',
          isForced: false,
          content: expect.stringContaining('欢迎'),
        },
      });
      expect(invokeTauri).toHaveBeenNthCalledWith(2, TauriCommand.mediaServer.uploadSubtitle, {
        serverId: 'srv-jellyfin-99',
        deleteExistingExternalIndex: undefined,
        payload: {
          itemId: 'item-101',
          language: 'en',
          format: 'srt',
          isForced: false,
          content: expect.stringContaining('Welcome'),
        },
      });
    });
  });
});
