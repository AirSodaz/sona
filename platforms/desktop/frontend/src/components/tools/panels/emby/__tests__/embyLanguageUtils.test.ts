import { describe, expect, it } from 'vitest';
import type { EmbyStreamInfo } from '../../../../../types/emby';
import {
  findBestAudioStream,
  getAudioTrackLabel,
  hasTargetSubtitle,
  matchSubtitleLanguage,
  normalizeLanguageCode,
} from '../embyLanguageUtils';

describe('embyLanguageUtils', () => {
  describe('normalizeLanguageCode', () => {
    it('normalizes undefined and empty inputs', () => {
      expect(normalizeLanguageCode(undefined)).toBe('');
      expect(normalizeLanguageCode('')).toBe('');
      expect(normalizeLanguageCode('   ')).toBe('');
    });

    it('normalizes Simplified/Generic Chinese variants to zh-CN', () => {
      expect(normalizeLanguageCode('chi')).toBe('zh-CN');
      expect(normalizeLanguageCode('zho')).toBe('zh-CN');
      expect(normalizeLanguageCode('chinese')).toBe('zh-CN');
      expect(normalizeLanguageCode('zh-cn')).toBe('zh-CN');
      expect(normalizeLanguageCode('zh-Hans')).toBe('zh-CN');
      expect(normalizeLanguageCode('chs')).toBe('zh-CN');
      expect(normalizeLanguageCode('cmn')).toBe('zh-CN');
      expect(normalizeLanguageCode('简体')).toBe('zh-CN');
    });

    it('normalizes Traditional Chinese variants to zh-TW', () => {
      expect(normalizeLanguageCode('zh-tw')).toBe('zh-TW');
      expect(normalizeLanguageCode('zh-hant')).toBe('zh-TW');
      expect(normalizeLanguageCode('cht')).toBe('zh-TW');
      expect(normalizeLanguageCode('traditional chinese')).toBe('zh-TW');
      expect(normalizeLanguageCode('繁体')).toBe('zh-TW');
    });

    it('normalizes other common language codes', () => {
      expect(normalizeLanguageCode('eng')).toBe('en');
      expect(normalizeLanguageCode('english')).toBe('en');
      expect(normalizeLanguageCode('jpn')).toBe('ja');
      expect(normalizeLanguageCode('japanese')).toBe('ja');
      expect(normalizeLanguageCode('kor')).toBe('ko');
      expect(normalizeLanguageCode('korean')).toBe('ko');
    });
  });

  describe('matchSubtitleLanguage', () => {
    it('matches language code for zh-CN', () => {
      expect(matchSubtitleLanguage('chi', undefined, 'zh-CN')).toBe(true);
      expect(matchSubtitleLanguage('zho', undefined, 'zh-CN')).toBe(true);
      expect(matchSubtitleLanguage('zh-cn', undefined, 'zh-CN')).toBe(true);
    });

    it('matches display title for zh-CN when language tag is missing or und', () => {
      expect(matchSubtitleLanguage('und', 'Chinese (Simplified)', 'zh-CN')).toBe(true);
      expect(matchSubtitleLanguage(undefined, '简中外挂', 'zh-CN')).toBe(true);
    });

    it('distinguishes Traditional Chinese from Simplified Chinese', () => {
      expect(matchSubtitleLanguage('chi', '繁體中文', 'zh-CN')).toBe(false);
      expect(matchSubtitleLanguage('chi', '繁體中文', 'zh-TW')).toBe(true);
      expect(matchSubtitleLanguage('zh-tw', undefined, 'zh-TW')).toBe(true);
      expect(matchSubtitleLanguage('zh-tw', undefined, 'zh-CN')).toBe(false);
    });

    it('matches Japanese and Korean by code or title', () => {
      expect(matchSubtitleLanguage('jpn', undefined, 'ja')).toBe(true);
      expect(matchSubtitleLanguage('und', 'Japanese [CC]', 'ja')).toBe(true);
      expect(matchSubtitleLanguage('kor', undefined, 'ko')).toBe(true);
      expect(matchSubtitleLanguage('und', '韩语内嵌', 'ko')).toBe(true);
    });

    it('returns false when no match', () => {
      expect(matchSubtitleLanguage('eng', 'English [SDH]', 'zh-CN')).toBe(false);
      expect(matchSubtitleLanguage(undefined, undefined, 'zh-CN')).toBe(false);
    });
  });

  describe('hasTargetSubtitle', () => {
    const embeddedChinese: EmbyStreamInfo = {
      index: 1,
      streamType: 'Subtitle',
      language: 'chi',
      displayTitle: 'Chinese',
      isExternal: false,
      isForced: false,
      isDefault: false,
    };

    const externalChinese: EmbyStreamInfo = {
      index: 2,
      streamType: 'Subtitle',
      language: 'zh-CN',
      displayTitle: '简中外挂',
      isExternal: true,
      isForced: false,
      isDefault: false,
    };

    const japaneseSub: EmbyStreamInfo = {
      index: 3,
      streamType: 'Subtitle',
      language: 'jpn',
      displayTitle: 'Japanese',
      isExternal: false,
      isForced: false,
      isDefault: false,
    };

    const audioStream: EmbyStreamInfo = {
      index: 0,
      streamType: 'Audio',
      language: 'eng',
      isExternal: false,
      isForced: false,
      isDefault: true,
    };

    it('returns false when mediaStreams is empty or undefined', () => {
      expect(hasTargetSubtitle(undefined, 'zh-CN')).toBe(false);
      expect(hasTargetSubtitle([], 'zh-CN')).toBe(false);
    });

    it('returns false when only non-matching subtitles or audio exist', () => {
      expect(hasTargetSubtitle([audioStream, japaneseSub], 'zh-CN')).toBe(false);
    });

    it('treats embedded and external subtitles equally as present (binary check)', () => {
      expect(hasTargetSubtitle([audioStream, embeddedChinese], 'zh-CN')).toBe(true);
      expect(hasTargetSubtitle([audioStream, externalChinese], 'zh-CN')).toBe(true);
    });

    it('supports dynamic target language toggling', () => {
      const streams = [audioStream, japaneseSub];
      // When target language is zh-CN, Japanese subtitle is not a match
      expect(hasTargetSubtitle(streams, 'zh-CN')).toBe(false);
      // When toggled to ja, Japanese subtitle matches
      expect(hasTargetSubtitle(streams, 'ja')).toBe(true);
    });
  });

  describe('findBestAudioStream', () => {
    it('returns undefined when no streams or no audio streams exist', () => {
      expect(findBestAudioStream(undefined)).toBeUndefined();
      expect(findBestAudioStream([])).toBeUndefined();
      expect(
        findBestAudioStream([
          {
            index: 0,
            streamType: 'Video',
            isExternal: false,
            isForced: false,
            isDefault: false,
          },
        ])
      ).toBeUndefined();
    });

    it('returns the stream marked as default', () => {
      const audio1: EmbyStreamInfo = {
        index: 1,
        streamType: 'Audio',
        language: 'eng',
        isExternal: false,
        isForced: false,
        isDefault: false,
      };
      const audio2: EmbyStreamInfo = {
        index: 2,
        streamType: 'Audio',
        language: 'jpn',
        isExternal: false,
        isForced: false,
        isDefault: true,
      };

      expect(findBestAudioStream([audio1, audio2])).toEqual(audio2);
    });

    it('falls back to the first audio stream when none is default', () => {
      const audio1: EmbyStreamInfo = {
        index: 1,
        streamType: 'Audio',
        language: 'eng',
        isExternal: false,
        isForced: false,
        isDefault: false,
      };
      const audio2: EmbyStreamInfo = {
        index: 2,
        streamType: 'Audio',
        language: 'jpn',
        isExternal: false,
        isForced: false,
        isDefault: false,
      };

      expect(findBestAudioStream([audio1, audio2])).toEqual(audio1);
    });
  });

  describe('getAudioTrackLabel', () => {
    it('uses displayTitle when available', () => {
      const stream: EmbyStreamInfo = {
        index: 1,
        streamType: 'Audio',
        displayTitle: 'Japanese Stereo AAC',
        isExternal: false,
        isForced: false,
        isDefault: true,
      };
      expect(getAudioTrackLabel(stream)).toBe('Japanese Stereo AAC (Default)');
    });

    it('falls back to language and codec', () => {
      const stream: EmbyStreamInfo = {
        index: 2,
        streamType: 'Audio',
        language: 'eng',
        codec: 'dts',
        isExternal: false,
        isForced: false,
        isDefault: false,
      };
      expect(getAudioTrackLabel(stream)).toBe('ENG DTS');
    });
  });
});
