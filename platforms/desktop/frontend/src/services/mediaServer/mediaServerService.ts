import type { MediaServerTaskContext } from '../../types/batchQueue';
import type {
  MediaServerInfo,
  MediaServerItemListResult,
  MediaServerLibrary,
  MediaServerLoginPayload,
  MediaServerLoginResult,
  MediaServerProfile,
  MediaServerSubtitleUploadPayload,
  MediaServerToolOverrides,
} from '../../types/mediaServer';
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
  let converted = text
    .replace(/\r\n|\r/g, '\n')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/(p|div)>/gi, '\n');
  let previous: string;
  do {
    previous = converted;
    converted = converted.replace(/<[^<>]*>/g, '');
  } while (converted !== previous);
  converted = converted
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

export async function testMediaServerConnection(
  profile: MediaServerProfile
): Promise<MediaServerInfo> {
  return invokeTauri(TauriCommand.mediaServer.testConnection, { profile });
}

export async function loginMediaServer(
  payload: MediaServerLoginPayload
): Promise<MediaServerLoginResult> {
  return invokeTauri(TauriCommand.mediaServer.login, { payload });
}

export async function listMediaServerProfiles(): Promise<MediaServerProfile[]> {
  return invokeTauri(TauriCommand.mediaServer.listProfiles);
}

export async function saveMediaServerProfile(profile: MediaServerProfile): Promise<void> {
  return invokeTauri(TauriCommand.mediaServer.saveProfile, { profile });
}

export async function deleteMediaServerProfile(profileId: string): Promise<void> {
  return invokeTauri(TauriCommand.mediaServer.deleteProfile, { profileId });
}

export async function setActiveMediaServerProfile(profileId: string): Promise<void> {
  return invokeTauri(TauriCommand.mediaServer.setActiveProfile, { profileId });
}

export async function getMediaServerLibraries(serverId?: string): Promise<MediaServerLibrary[]> {
  return invokeTauri(TauriCommand.mediaServer.getLibraries, { serverId });
}

export async function getMediaServerItems(params: {
  serverId?: string;
  parentId?: string;
  searchTerm?: string;
  startIndex?: number;
  limit?: number;
}): Promise<MediaServerItemListResult> {
  return invokeTauri(TauriCommand.mediaServer.getItems, {
    serverId: params.serverId,
    parentId: params.parentId,
    searchTerm: params.searchTerm,
    startIndex: params.startIndex,
    limit: params.limit,
  });
}

export async function downloadMediaServerAudio(
  itemId: string,
  streamIndex?: number,
  serverId?: string,
  filename?: string
): Promise<string> {
  return invokeTauri(TauriCommand.mediaServer.downloadAudio, {
    itemId,
    streamIndex,
    serverId,
    filename,
  });
}

export async function uploadMediaServerSubtitle(
  payload: MediaServerSubtitleUploadPayload,
  deleteExistingExternalIndex?: number,
  serverId?: string
): Promise<void> {
  return invokeTauri(TauriCommand.mediaServer.uploadSubtitle, {
    payload,
    deleteExistingExternalIndex,
    serverId,
  });
}

export async function getMediaServerProcessingSettings(): Promise<MediaServerToolOverrides | null> {
  return invokeTauri(TauriCommand.mediaServer.getProcessingSettings);
}

export async function saveMediaServerProcessingSettings(
  settings: MediaServerToolOverrides
): Promise<void> {
  return invokeTauri(TauriCommand.mediaServer.saveProcessingSettings, { settings });
}

export async function syncMediaServerSubtitles(
  segments: TranscriptSegment[],
  context: MediaServerTaskContext,
  options?: { deleteExistingIndex?: number }
): Promise<void> {
  const targetLang = context.targetLanguage || 'zh-CN';
  const deleteIndex = options?.deleteExistingIndex ?? context.existingExternalSubtitleIndex;

  switch (context.subtitleMode) {
    case 'pure_translation': {
      const content = generatePureTranslationSrt(segments);
      if (!content.trim()) return;
      await uploadMediaServerSubtitle(
        {
          itemId: context.itemId,
          language: targetLang,
          format: 'srt',
          isForced: false,
          content,
        },
        deleteIndex,
        context.serverId
      );
      break;
    }
    case 'bilingual': {
      const content = generateBilingualSrt(segments);
      if (!content.trim()) return;
      await uploadMediaServerSubtitle(
        {
          itemId: context.itemId,
          language: targetLang,
          format: 'srt',
          isForced: false,
          content,
        },
        deleteIndex,
        context.serverId
      );
      break;
    }
    case 'separate_tracks': {
      const hasTranslation = segments.some((seg) =>
        Boolean(cleanSubtitleText(seg.translation || ''))
      );
      const transContent = generatePureTranslationSrt(segments);
      if (hasTranslation && transContent.trim()) {
        await uploadMediaServerSubtitle(
          {
            itemId: context.itemId,
            language: targetLang,
            format: 'srt',
            isForced: false,
            content: transContent,
          },
          deleteIndex,
          context.serverId
        );
      }
      const origContent = generateOriginalSrt(segments);
      if (origContent.trim()) {
        await uploadMediaServerSubtitle(
          {
            itemId: context.itemId,
            language: context.sourceLanguage || 'und',
            format: 'srt',
            isForced: false,
            content: origContent,
          },
          undefined,
          context.serverId
        );
      }
      break;
    }
    default:
      throw new Error(`Unsupported media server subtitle mode: ${String(context.subtitleMode)}`);
  }
}
