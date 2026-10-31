import type { AsrModelSelection } from './config';

export type MediaServerType = 'emby' | 'jellyfin';

export type MediaServerSubtitleMode = 'pure_translation' | 'bilingual' | 'separate_tracks';

export interface MediaServerProfile {
  id: string;
  name: string;
  serverType: MediaServerType;
  serverUrl: string;
  apiKey: string;
  username?: string;
  adminUserId?: string;
  allowInvalidCerts: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface MediaServerLoginPayload {
  serverUrl: string;
  serverType: MediaServerType;
  username: string;
  password?: string;
  allowInvalidCerts: boolean;
}

export interface MediaServerLoginResult {
  accessToken: string;
  userId: string;
  userName: string;
  isAdmin: boolean;
}

export interface MediaServerInfo {
  id: string;
  serverName: string;
  version: string;
  hasSubtitleManagerPlugin: boolean;
  adminUserId?: string;
  username?: string;
  serverType?: MediaServerType;
}

export interface MediaServerLibrary {
  id: string;
  name: string;
  collectionType?: string;
}

export interface MediaServerStreamInfo {
  index: number;
  streamType: string;
  language?: string;
  displayTitle?: string;
  codec?: string;
  isExternal: boolean;
  isForced: boolean;
  isDefault: boolean;
}

export interface MediaServerMediaItem {
  id: string;
  name: string;
  seriesName?: string;
  seasonName?: string;
  seasonIndex?: number;
  episodeIndex?: number;
  runtimeTicks?: number;
  mediaStreams: MediaServerStreamInfo[];
  hasChineseSubtitles: boolean;
  hasEmbeddedChinese: boolean;
  itemType?: string;
  mediaSourceId?: string;
  container?: string;
}

export interface MediaServerItemListResult {
  items: MediaServerMediaItem[];
  totalRecordCount: number;
}

export interface MediaServerSubtitleUploadPayload {
  itemId: string;
  language: string;
  format: string;
  isForced: boolean;
  content: string;
}

export interface MediaServerToolOverrides {
  asrSelection?: AsrModelSelection | null;
  asrLanguage?: string;
  translationModelId?: string | null;
  defaultSubtitleMode?: MediaServerSubtitleMode;
  overwriteExistingExternal?: boolean;
}

export const DEFAULT_MEDIA_SERVER_TOOL_OVERRIDES: MediaServerToolOverrides = {
  asrLanguage: 'auto',
  defaultSubtitleMode: 'bilingual',
  overwriteExistingExternal: true,
};
