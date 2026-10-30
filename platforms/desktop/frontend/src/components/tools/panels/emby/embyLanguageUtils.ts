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
  const trimmed = code.trim().replace(/_/g, '-').toLowerCase();

  // Traditional Chinese checks first
  if (
    [
      'zh-tw',
      'zh-hant',
      'zh-hk',
      'zh-mo',
      'zh-cht',
      'cht',
      'zht',
      'traditional chinese',
      '繁体',
      '繁體',
      '繁體中文',
      '繁体中文',
      '繁中',
      '繁',
    ].includes(trimmed) ||
    trimmed.startsWith('zh-tw') ||
    trimmed.startsWith('zh-hant') ||
    trimmed.startsWith('zh-hk') ||
    trimmed.startsWith('zh-mo') ||
    trimmed.startsWith('zh-cht') ||
    trimmed.startsWith('cmn-hant') ||
    trimmed.startsWith('cmn-tw') ||
    trimmed.startsWith('zho-hant')
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
      'zh-sg',
      'zh-hans',
      'zh-chs',
      'chs',
      'cmn',
      'simplified chinese',
      '简体',
      '简体中文',
      '中文',
      '国语',
      '普通话',
      '简中',
      '簡中',
      '簡體',
      '簡體中文',
      '簡',
      '简',
      'cmn-cn',
      'cmn-hans',
      'zho-hans',
    ].includes(trimmed) ||
    trimmed.startsWith('zh-cn') ||
    trimmed.startsWith('zh-sg') ||
    trimmed.startsWith('zh-hans') ||
    trimmed.startsWith('zh-chs') ||
    trimmed.startsWith('cmn-hans') ||
    trimmed.startsWith('cmn-cn') ||
    trimmed.startsWith('zho-hans')
  ) {
    return 'zh-CN';
  }
  // English
  if (trimmed === 'en' || trimmed.startsWith('en-') || ['eng', 'english'].includes(trimmed)) {
    return 'en';
  }

  // Japanese
  if (
    trimmed === 'ja' ||
    trimmed.startsWith('ja-') ||
    ['jpn', 'japanese', '日本語', '日文', '日语'].includes(trimmed)
  ) {
    return 'ja';
  }

  // Korean
  if (
    trimmed === 'ko' ||
    trimmed.startsWith('ko-') ||
    ['kor', 'korean', '한국어', '韩文', '韩语'].includes(trimmed)
  ) {
    return 'ko';
  }

  // French
  if (
    trimmed === 'fr' ||
    trimmed.startsWith('fr-') ||
    ['fra', 'fre', 'french', 'français'].includes(trimmed)
  ) {
    return 'fr';
  }

  // German
  if (
    trimmed === 'de' ||
    trimmed.startsWith('de-') ||
    ['deu', 'ger', 'german', 'deutsch'].includes(trimmed)
  ) {
    return 'de';
  }

  // Spanish
  if (
    trimmed === 'es' ||
    trimmed.startsWith('es-') ||
    ['spa', 'spanish', 'español'].includes(trimmed)
  ) {
    return 'es';
  }

  // Russian
  if (
    trimmed === 'ru' ||
    trimmed.startsWith('ru-') ||
    ['rus', 'russian', 'русский'].includes(trimmed)
  ) {
    return 'ru';
  }

  // Cantonese
  if (
    ['yue', 'cantonese', '粤语', '粵語', '粤', '粵', 'zh-yue'].includes(trimmed) ||
    trimmed.startsWith('zh-yue') ||
    trimmed.startsWith('yue-')
  ) {
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

  const isStreamChinese =
    normStreamLang === 'zh-CN' || normStreamLang === 'zh-TW' || normStreamLang === 'yue';
  const isTargetChinese = normTarget === 'zh-CN' || normTarget === 'zh-TW' || normTarget === 'yue';
  const isRecognizedStreamLang = Boolean(
    normStreamLang && !['und', 'unknown', 'unk', 'mul', 'mis', 'zxx', ''].includes(normStreamLang)
  );

  // If stream language is an explicit recognized language distinct from target language
  // (and neither is Chinese), prevent title heuristics from overriding the container tag.
  if (
    isRecognizedStreamLang &&
    normStreamLang !== normTarget &&
    !(isStreamChinese && isTargetChinese)
  ) {
    return false;
  }

  // 1. If target is Traditional Chinese (zh-TW)
  if (normTarget === 'zh-TW') {
    const hasTraditionalMarker =
      titleLower.includes('繁体') ||
      titleLower.includes('繁體') ||
      titleLower.includes('繁中') ||
      titleLower.includes('繁简') ||
      titleLower.includes('简繁') ||
      titleLower.includes('繁簡') ||
      titleLower.includes('簡繁') ||
      titleLower.includes('zh-hk') ||
      titleLower.includes('zh-mo') ||
      titleLower.includes('zh-tw') ||
      titleLower.includes('zh-hant') ||
      titleLower.includes('traditional') ||
      /(?:^|[^a-z0-9])(cht|zht|big5)(?:[^a-z0-9]|$)/i.test(titleLower) ||
      /(?:^|[^a-z0-9\u4e00-\u9fa5])[[(【（\s._-]?繁[\])】）\s._-]?(?:$|[^a-z0-9\u4e00-\u9fa5])/i.test(
        titleLower
      );

    const hasSimplifiedMarker =
      titleLower.includes('简体') ||
      titleLower.includes('簡體') ||
      titleLower.includes('简中') ||
      titleLower.includes('簡中') ||
      titleLower.includes('繁简') ||
      titleLower.includes('简繁') ||
      titleLower.includes('繁簡') ||
      titleLower.includes('簡繁') ||
      titleLower.includes('zh-cn') ||
      titleLower.includes('zh-sg') ||
      titleLower.includes('zh-hans') ||
      titleLower.includes('zh-chs') ||
      titleLower.includes('simplified') ||
      /(?:^|[^a-z0-9])(chs|gb2312|gbk|gb18030)(?:[^a-z0-9]|$)/i.test(titleLower) ||
      /(?:^|[^a-z0-9\u4e00-\u9fa5])[[(【（\s._-]?([简簡])[\])】）\s._-]?(?:$|[^a-z0-9\u4e00-\u9fa5])/i.test(
        titleLower
      );
    if (hasSimplifiedMarker && !hasTraditionalMarker) {
      return false;
    }
    if (normStreamLang === 'zh-CN') {
      return hasTraditionalMarker;
    }
    if (normStreamLang === 'zh-TW') return true;
    if (hasTraditionalMarker) {
      return true;
    }
    return false;
  }

  // 2. If target is Simplified Chinese (zh-CN)
  if (normTarget === 'zh-CN') {
    const hasTraditionalMarker =
      titleLower.includes('繁体') ||
      titleLower.includes('繁體') ||
      titleLower.includes('繁中') ||
      titleLower.includes('繁简') ||
      titleLower.includes('简繁') ||
      titleLower.includes('繁簡') ||
      titleLower.includes('簡繁') ||
      titleLower.includes('zh-hk') ||
      titleLower.includes('zh-mo') ||
      titleLower.includes('zh-tw') ||
      titleLower.includes('zh-hant') ||
      titleLower.includes('traditional') ||
      /(?:^|[^a-z0-9])(cht|zht|big5)(?:[^a-z0-9]|$)/i.test(titleLower) ||
      /(?:^|[^a-z0-9\u4e00-\u9fa5])[[(【（\s._-]?繁[\])】）\s._-]?(?:$|[^a-z0-9\u4e00-\u9fa5])/i.test(
        titleLower
      );

    const hasSimplifiedMarker =
      titleLower.includes('简体') ||
      titleLower.includes('簡體') ||
      titleLower.includes('简中') ||
      titleLower.includes('簡中') ||
      titleLower.includes('繁简') ||
      titleLower.includes('简繁') ||
      titleLower.includes('繁簡') ||
      titleLower.includes('簡繁') ||
      titleLower.includes('zh-cn') ||
      titleLower.includes('zh-sg') ||
      titleLower.includes('zh-hans') ||
      titleLower.includes('zh-chs') ||
      titleLower.includes('simplified') ||
      /(?:^|[^a-z0-9])(chs|gb2312|gbk|gb18030)(?:[^a-z0-9]|$)/i.test(titleLower) ||
      /(?:^|[^a-z0-9\u4e00-\u9fa5])[[(【（\s._-]?([简簡])[\])】）\s._-]?(?:$|[^a-z0-9\u4e00-\u9fa5])/i.test(
        titleLower
      );
    // Exclude only if title explicitly specifies Traditional without Simplified markers
    if (hasTraditionalMarker && !hasSimplifiedMarker) {
      return false;
    }
    if (normStreamLang === 'zh-TW' || normStreamLang === 'yue') {
      return hasSimplifiedMarker;
    }
    if (normStreamLang === 'zh-CN') return true;
    if (
      hasSimplifiedMarker ||
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
    if (
      normTarget === 'en' &&
      (titleLower.includes('english') || /(?:^|[^a-z0-9])eng(?:[^a-z0-9]|$)/i.test(titleLower))
    ) {
      return true;
    }
    if (
      normTarget === 'ja' &&
      (titleLower.includes('japanese') ||
        /(?:^|[^a-z0-9])jpn(?:[^a-z0-9]|$)/i.test(titleLower) ||
        titleLower.includes('日文') ||
        titleLower.includes('日语') ||
        titleLower.includes('日本語'))
    ) {
      return true;
    }
    if (
      normTarget === 'ko' &&
      (titleLower.includes('korean') ||
        /(?:^|[^a-z0-9])kor(?:[^a-z0-9]|$)/i.test(titleLower) ||
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
  const cleanTitle = stream.displayTitle?.trim();
  if (cleanTitle) {
    parts.push(cleanTitle);
  } else {
    if (stream.language) parts.push(stream.language.toUpperCase());
    if (stream.codec) parts.push(stream.codec.toUpperCase());
  }
  if (parts.length === 0) {
    parts.push(`Track #${stream.index}`);
  }
  const hasDefaultInTitle = Boolean(cleanTitle?.toLowerCase().includes('default'));
  if (stream.isDefault && !hasDefaultInTitle) {
    parts.push('(Default)');
  }
  return parts.join(' ').trim() || `Track #${stream.index}`;
}
