export interface EmbyConnectionConfig {
  serverUrl: string;
  apiKey: string;
  allowInvalidCerts: boolean;
}

export interface EmbyServerInfo {
  id: string;
  serverName: string;
  version: string;
  hasSubtitleManagerPlugin: boolean;
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
