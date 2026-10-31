import type {
  MediaServerInfo,
  MediaServerItemListResult,
  MediaServerLibrary,
  MediaServerMediaItem,
  MediaServerStreamInfo,
  MediaServerSubtitleMode,
  MediaServerSubtitleUploadPayload,
  MediaServerToolOverrides,
} from './mediaServer';

export type EmbySubtitleMode = MediaServerSubtitleMode;
export type EmbyServerInfo = MediaServerInfo;
export type EmbyLibrary = MediaServerLibrary;
export type EmbyStreamInfo = MediaServerStreamInfo;
export type EmbyMediaItem = MediaServerMediaItem;
export type EmbyItemListResult = MediaServerItemListResult;
export type SubtitleUploadPayload = MediaServerSubtitleUploadPayload;
export type EmbyToolOverrides = MediaServerToolOverrides;
export { DEFAULT_MEDIA_SERVER_TOOL_OVERRIDES as DEFAULT_EMBY_TOOL_OVERRIDES } from './mediaServer';

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
