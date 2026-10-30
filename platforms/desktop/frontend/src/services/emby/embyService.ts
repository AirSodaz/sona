import type { EmbyTaskContext } from '../../types/batchQueue';
import type {
  EmbyConnectionConfig,
  EmbyItemListResult,
  EmbyLibrary,
  EmbyLoginPayload,
  EmbyServerInfo,
  SubtitleUploadPayload,
} from '../../types/emby';
import type { TranscriptSegment } from '../../types/transcript';
import { TauriCommand } from '../tauri/commands';
import { invokeTauri } from '../tauri/invoke';
export function formatSrtTimestamp(seconds: number): string {
  const clamped = Math.max(0, Number.isFinite(seconds) ? seconds : 0);
  const totalMillis = Math.round(clamped * 1000);
  const hours = Math.floor(totalMillis / 3600000);
  const minutes = Math.floor((totalMillis % 3600000) / 60000);
  const secs = Math.floor((totalMillis % 60000) / 1000);
  const millis = totalMillis % 1000;
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')},${String(millis).padStart(3, '0')}`;
}

export function cleanSubtitleText(text: string): string {
  if (!text || typeof text !== 'string') return '';
  const converted = text
    .replace(/\r\n|\r/g, '\n')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/(p|div)>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&apos;/g, "'")
    .replace(/&#x27;/gi, "'")
    .replace(/&amp;/g, '&');
  return converted
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .join('\n');
}

/**
 * Format segments into SRT format.
 */
function buildSrt(
  segments: TranscriptSegment[],
  textExtractor: (seg: TranscriptSegment) => string
): string {
  const blocks: string[] = [];
  let index = 1;

  for (const seg of segments) {
    if (seg.isFinal === false) continue;
    const text = textExtractor(seg).trim();
    if (!text) continue;
    const startSec = Math.max(0, Number.isFinite(seg.start) ? seg.start : 0);
    const endSec = Math.max(startSec + 0.05, Number.isFinite(seg.end) ? seg.end : startSec + 0.05);
    const start = formatSrtTimestamp(startSec);
    const end = formatSrtTimestamp(endSec);
    blocks.push(`${index}\n${start} --> ${end}\n${text}`);
    index++;
  }

  return blocks.length > 0 ? `${blocks.join('\n\n')}\n` : '';
}

export function generatePureTranslationSrt(segments: TranscriptSegment[]): string {
  return buildSrt(segments, (seg) => {
    const trans = cleanSubtitleText(seg.translation || '');
    return trans || cleanSubtitleText(seg.text || '');
  });
}

export function generateOriginalSrt(segments: TranscriptSegment[]): string {
  return buildSrt(segments, (seg) => cleanSubtitleText(seg.text || ''));
}

export function generateBilingualSrt(segments: TranscriptSegment[]): string {
  return buildSrt(segments, (seg) => {
    const trans = cleanSubtitleText(seg.translation || '');
    const orig = cleanSubtitleText(seg.text || '');
    if (trans && orig && trans.toLowerCase() !== orig.toLowerCase()) {
      return `${trans}\n${orig}`;
    }
    return trans || orig;
  });
}

export async function loginEmby(payload: EmbyLoginPayload): Promise<EmbyServerInfo> {
  return invokeTauri(TauriCommand.emby.login, { payload });
}

export async function testEmbyConnection(config: EmbyConnectionConfig): Promise<EmbyServerInfo> {
  return invokeTauri(TauriCommand.emby.testConnection, { config });
}

export async function saveEmbyConfig(config: EmbyConnectionConfig): Promise<void> {
  return invokeTauri(TauriCommand.emby.saveConfig, { config });
}

export async function getEmbyConfig(): Promise<EmbyConnectionConfig | null> {
  return invokeTauri(TauriCommand.emby.getConfig);
}

export async function getEmbyLibraries(): Promise<EmbyLibrary[]> {
  return invokeTauri(TauriCommand.emby.getLibraries);
}

export async function getEmbyItems(params: {
  parentId?: string;
  searchTerm?: string;
  startIndex: number;
  limit: number;
}): Promise<EmbyItemListResult> {
  return invokeTauri(TauriCommand.emby.getItems, {
    parentId: params.parentId,
    searchTerm: params.searchTerm,
    startIndex: params.startIndex,
    limit: params.limit,
  });
}

export async function downloadEmbyAudio(itemId: string, streamIndex?: number): Promise<string> {
  return invokeTauri(TauriCommand.emby.downloadAudio, {
    itemId,
    streamIndex,
  });
}
export async function uploadEmbySubtitle(
  payload: SubtitleUploadPayload,
  deleteExistingExternalIndex?: number
): Promise<void> {
  return invokeTauri(TauriCommand.emby.uploadSubtitle, {
    payload,
    deleteExistingExternalIndex,
  });
}

export async function syncEmbySubtitles(
  segments: TranscriptSegment[],
  embyContext: EmbyTaskContext,
  options?: { deleteExistingIndex?: number }
): Promise<void> {
  const targetLang = embyContext.targetLanguage || 'zh-CN';
  const deleteIndex = options?.deleteExistingIndex ?? embyContext.existingExternalSubtitleIndex;

  switch (embyContext.subtitleMode) {
    case 'pure_translation': {
      const content = generatePureTranslationSrt(segments);
      if (!content.trim()) return;
      await uploadEmbySubtitle(
        {
          itemId: embyContext.itemId,
          language: targetLang,
          format: 'srt',
          isForced: false,
          content,
        },
        deleteIndex
      );
      break;
    }
    case 'bilingual': {
      const content = generateBilingualSrt(segments);
      if (!content.trim()) return;
      await uploadEmbySubtitle(
        {
          itemId: embyContext.itemId,
          language: targetLang,
          format: 'srt',
          isForced: false,
          content,
        },
        deleteIndex
      );
      break;
    }
    case 'separate_tracks': {
      const hasTranslation = segments.some((seg) =>
        Boolean(cleanSubtitleText(seg.translation || ''))
      );
      const transContent = generatePureTranslationSrt(segments);
      if (hasTranslation && transContent.trim()) {
        await uploadEmbySubtitle(
          {
            itemId: embyContext.itemId,
            language: targetLang,
            format: 'srt',
            isForced: false,
            content: transContent,
          },
          deleteIndex
        );
      }
      const origContent = generateOriginalSrt(segments);
      if (origContent.trim()) {
        await uploadEmbySubtitle(
          {
            itemId: embyContext.itemId,
            language: embyContext.sourceLanguage || 'und',
            format: 'srt',
            isForced: false,
            content: origContent,
          },
          undefined
        );
      }
      break;
    }
    default:
      throw new Error(`Unsupported Emby subtitle mode: ${String(embyContext.subtitleMode)}`);
  }
}
