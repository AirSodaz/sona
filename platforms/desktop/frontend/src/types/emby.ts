import type { EmbySubtitleMode } from './batchQueue';
import type { AsrModelSelection } from './config';

export interface EmbyConnectionConfig {
  serverUrl: string;
  apiKey: string;
  username?: string;
  allowInvalidCerts: boolean;
  adminUserId?: string;
}

export interface EmbyLoginPayload {
  serverUrl: string;
  username: string;
  password?: string;
  allowInvalidCerts: boolean;
}

export interface EmbyServerInfo {
  id: string;
  serverName: string;
  version: string;
  hasSubtitleManagerPlugin: boolean;
  adminUserId?: string;
  username?: string;
}
export interface EmbyLibrary {
  id: string;
  name: string;
  collectionType?: string;
}

export interface EmbyStreamInfo {
  index: number;
  streamType: string;
  language?: string;
  displayTitle?: string;
  codec?: string;
  isExternal: boolean;
  isForced: boolean;
  isDefault: boolean;
}

export interface EmbyMediaItem {
  id: string;
  name: string;
  seriesName?: string;
  seasonName?: string;
  seasonIndex?: number;
  episodeIndex?: number;
  runtimeTicks?: number;
  mediaStreams: EmbyStreamInfo[];
  hasChineseSubtitles: boolean;
  hasEmbeddedChinese: boolean;
  itemType?: string;
  mediaSourceId?: string;
  container?: string;
}

export interface EmbyItemListResult {
  items: EmbyMediaItem[];
  totalRecordCount: number;
}

export interface SubtitleUploadPayload {
  itemId: string;
  language: string;
  format: string;
  isForced: boolean;
  content: string;
}

export interface EmbyToolOverrides {
  asrSelection?: AsrModelSelection | null;
  asrLanguage?: string;
  translationModelId?: string | null;
  defaultSubtitleMode?: EmbySubtitleMode;
  overwriteExistingExternal?: boolean;
}
