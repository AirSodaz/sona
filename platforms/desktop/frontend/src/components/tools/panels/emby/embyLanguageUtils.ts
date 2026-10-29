import type { EmbyStreamInfo } from '../../../../types/emby';

/**
 * Normalizes 2-letter, 3-letter, or verbal language names to standard codes
 * e.g., 'chi', 'zho', 'chinese', 'zh-cn', 'cmn' -> 'zh-CN'
 *       'zho', 'zh-tw', 'zh-hant' -> 'zh-TW'
 *       'eng', 'english' -> 'en'
 *       'jpn', 'japanese' -> 'ja'
 *       'kor', 'korean' -> 'ko'
 */
export function normalizeLanguageCode(code?: string): string {
  if (!code) return '';
  const trimmed = code.trim().toLowerCase();

  // Traditional Chinese checks first
  if (
    [
      'zh-tw',
      'zh-hant',
      'zh-hk',
      'cht',
      'zht',
      'traditional chinese',
      '繁体',
      '繁體',
      '繁體中文',
      '繁体中文',
    ].includes(trimmed)
  ) {
    return 'zh-TW';
  }

  // Simplified / Generic Chinese
  if (
    [
      'zh',
      'chi',
      'zho',
      'chinese',
      'zh-cn',
      'zh-hans',
      'chs',
      'cmn',
      'simplified chinese',
      '简体',
      '简体中文',
      '中文',
      '国语',
      '普通话',
    ].includes(trimmed)
  ) {
    return 'zh-CN';
  }

  // English
  if (['en', 'eng', 'english'].includes(trimmed)) {
    return 'en';
  }

  // Japanese
  if (['ja', 'jpn', 'japanese', '日本語', '日文', '日语'].includes(trimmed)) {
    return 'ja';
  }

  // Korean
  if (['ko', 'kor', 'korean', '한국어', '韩文', '韩语'].includes(trimmed)) {
    return 'ko';
  }

  // French
  if (['fr', 'fra', 'fre', 'french', 'français'].includes(trimmed)) {
    return 'fr';
  }

  // German
  if (['de', 'deu', 'ger', 'german', 'deutsch'].includes(trimmed)) {
    return 'de';
  }

  // Spanish
  if (['es', 'spa', 'spanish', 'español'].includes(trimmed)) {
    return 'es';
  }

  // Russian
  if (['ru', 'rus', 'russian', 'русский'].includes(trimmed)) {
    return 'ru';
  }

  // Cantonese
  if (['yue', 'cantonese', '粤语'].includes(trimmed)) {
    return 'yue';
  }

  return trimmed;
}

/**
 * Checks whether a stream's language code or display title matches the target language.
 */
export function matchSubtitleLanguage(
  streamLanguage?: string,
  streamDisplayTitle?: string,
  targetLanguage?: string
): boolean {
  if (!targetLanguage) return false;

  const normTarget = normalizeLanguageCode(targetLanguage);
  const normStreamLang = normalizeLanguageCode(streamLanguage);
  const titleLower = streamDisplayTitle ? streamDisplayTitle.toLowerCase() : '';

  // 1. If target is Traditional Chinese (zh-TW)
  if (normTarget === 'zh-TW') {
    if (normStreamLang === 'zh-TW') return true;
    if (
      titleLower.includes('繁') ||
      titleLower.includes('cht') ||
      titleLower.includes('traditional') ||
      titleLower.includes('zh-tw') ||
      titleLower.includes('zh-hant') ||
      titleLower.includes('big5')
    ) {
      return true;
    }
    return false;
  }

  // 2. If target is Simplified Chinese (zh-CN)
  if (normTarget === 'zh-CN') {
    // Exclude if title explicitly specifies Traditional
    if (
      titleLower.includes('繁') ||
      titleLower.includes('cht') ||
      titleLower.includes('traditional') ||
      titleLower.includes('zh-tw') ||
      titleLower.includes('zh-hant') ||
      titleLower.includes('big5')
    ) {
      return false;
    }
    if (normStreamLang === 'zh-CN') return true;
    if (
      titleLower.includes('简') ||
      titleLower.includes('chs') ||
      titleLower.includes('simplified') ||
      titleLower.includes('zh-cn') ||
      titleLower.includes('zh-hans') ||
      titleLower.includes('gb') ||
      titleLower.includes('中文') ||
      titleLower.includes('chinese') ||
      titleLower.includes('国语')
    ) {
      return true;
    }
    return false;
  }

  // 3. For any other language (e.g. en, ja, ko)
  if (normStreamLang && normStreamLang === normTarget) {
    return true;
  }

  if (titleLower) {
    if (normTarget === 'en' && (titleLower.includes('english') || titleLower.includes('eng'))) {
      return true;
    }
    if (
      normTarget === 'ja' &&
      (titleLower.includes('japanese') ||
        titleLower.includes('jpn') ||
        titleLower.includes('日文') ||
        titleLower.includes('日语') ||
        titleLower.includes('日本語'))
    ) {
      return true;
    }
    if (
      normTarget === 'ko' &&
      (titleLower.includes('korean') ||
        titleLower.includes('kor') ||
        titleLower.includes('韩文') ||
        titleLower.includes('韩语') ||
        titleLower.includes('한국어'))
    ) {
      return true;
    }
  }

  return false;
}

/**
 * Evaluates whether any stream with streamType === 'subtitle' matches targetLanguage.
 * External and embedded subtitles are treated equally as 'present' (binary check).
 */
export function hasTargetSubtitle(
  mediaStreams: EmbyStreamInfo[] | undefined,
  targetLanguage: string
): boolean {
  if (!mediaStreams || mediaStreams.length === 0) return false;
  const subtitleStreams = mediaStreams.filter(
    (stream) => stream.streamType?.toLowerCase() === 'subtitle'
  );
  return subtitleStreams.some((stream) =>
    matchSubtitleLanguage(stream.language, stream.displayTitle, targetLanguage)
  );
}

/**
 * Finds the stream with isDefault === true, falling back to the first audio stream if none is default.
 */
export function findBestAudioStream(
  mediaStreams: EmbyStreamInfo[] | undefined
): EmbyStreamInfo | undefined {
  if (!mediaStreams || mediaStreams.length === 0) return undefined;
  const audios = mediaStreams.filter((stream) => stream.streamType?.toLowerCase() === 'audio');
  if (audios.length === 0) return undefined;
  const defaultStream = audios.find((stream) => stream.isDefault);
  return defaultStream ?? audios[0];
}

/**
 * Builds user-friendly audio track label with display title, language, codec, and default badge.
 */
export function getAudioTrackLabel(stream: EmbyStreamInfo): string {
  const parts: string[] = [];
  if (stream.displayTitle) {
    parts.push(stream.displayTitle);
  } else {
    if (stream.language) parts.push(stream.language.toUpperCase());
    if (stream.codec) parts.push(stream.codec.toUpperCase());
  }
  if (stream.isDefault) parts.push('(Default)');
  return parts.join(' ') || `Track #${stream.index}`;
}
