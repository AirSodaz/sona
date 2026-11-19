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
  buildMediaServerSubtitleUploadPlans,
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
  const plans = buildMediaServerSubtitleUploadPlans(segments, embyContext, options);
  for (const plan of plans) {
    await uploadEmbySubtitle(plan.payload, plan.deleteExistingIndex);
  }
}
