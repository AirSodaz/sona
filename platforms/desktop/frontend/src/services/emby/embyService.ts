import type { EmbyTaskContext } from '../../types/batchQueue';
import type {
  EmbyConnectionConfig,
  EmbyItemListResult,
  EmbyLibrary,
  EmbyLoginPayload,
  EmbyServerInfo,
  EmbyToolOverrides,
  SubtitleUploadPayload,
} from '../../types/emby';
import type { TranscriptSegment } from '../../types/transcript';
import {
  cleanSubtitleText,
  formatSrtTimestamp,
  generateBilingualSrt,
  generateOriginalSrt,
  generatePureTranslationSrt,
} from '../mediaServer/mediaServerService';
import { TauriCommand } from '../tauri/commands';
import { invokeTauri } from '../tauri/invoke';

export {
  cleanSubtitleText,
  formatSrtTimestamp,
  generateBilingualSrt,
  generateOriginalSrt,
  generatePureTranslationSrt,
};

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

export async function saveEmbyProcessingSettings(settings: EmbyToolOverrides): Promise<void> {
  return invokeTauri(TauriCommand.emby.saveProcessingSettings, { settings });
}

export async function getEmbyProcessingSettings(): Promise<EmbyToolOverrides | null> {
  return invokeTauri(TauriCommand.emby.getProcessingSettings);
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
