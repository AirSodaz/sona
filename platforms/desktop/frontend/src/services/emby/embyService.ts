import type {
  EmbyConnectionConfig,
  EmbyItemListResult,
  EmbyLibrary,
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
  return text
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/p>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&')
    .trim();
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

    const start = formatSrtTimestamp(seg.start);
    const end = formatSrtTimestamp(seg.end);
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
    if (trans && orig) {
      return `${trans}\n${orig}`;
    }
    return trans || orig;
  });
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

export async function downloadEmbyAudio(
  itemId: string,
  streamIndex?: number,
  tempDir?: string
): Promise<string> {
  return invokeTauri(TauriCommand.emby.downloadAudio, {
    itemId,
    streamIndex,
    tempDir,
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
