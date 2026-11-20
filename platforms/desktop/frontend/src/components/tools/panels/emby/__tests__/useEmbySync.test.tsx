import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as EmbyServiceModule from '../../../../../services/emby/embyService';
import * as embyService from '../../../../../services/emby/embyService';
import { saveEmbyToolOverrides } from '../../../../../services/emby/embySettingsService';
import type * as MediaServerServiceModule from '../../../../../services/mediaServer/mediaServerService';
import * as mediaServerService from '../../../../../services/mediaServer/mediaServerService';
import { useBatchQueueStore } from '../../../../../stores/batchQueueStore';
import { useConfigStore } from '../../../../../stores/configStore';
import { useToolsStore } from '../../../../../stores/toolsStore';
import { useTranscriptStore } from '../../../../../stores/transcriptStore';
import type { MediaServerProfile } from '../../../../../types/mediaServer';
import { useEmbySync } from '../useEmbySync';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? key,
      i18n: { language: 'zh-CN' },
    }),
  };
});

vi.mock('../../../../../services/mediaServer/mediaServerService', async (importOriginal) => {
  const actual = await importOriginal<typeof MediaServerServiceModule>();
  return {
    ...actual,
    listMediaServerProfiles: vi.fn(),
    saveMediaServerProfile: vi.fn(),
    setActiveMediaServerProfile: vi.fn(),
    testMediaServerConnection: vi.fn(),
    getMediaServerLibraries: vi.fn(),
    getMediaServerItems: vi.fn(),
  };
});

vi.mock('../../../../../services/emby/embyService', async (importOriginal) => {
  const actual = await importOriginal<typeof EmbyServiceModule>();
  return {
    ...actual,
    getEmbyConfig: vi.fn(),
    saveEmbyConfig: vi.fn(),
    testEmbyConnection: vi.fn(),
    loginEmby: vi.fn(),
    getEmbyLibraries: vi.fn(),
    getEmbyItems: vi.fn(),
    getEmbyProcessingSettings: vi.fn(),
  };
});

vi.mock('../../../../../services/recoveryService', () => ({
  persistQueueRecoverySnapshot: vi.fn(),
  toBatchQueueItem: vi.fn((x) => x),
}));

const embyProfile: MediaServerProfile = {
  id: 'default-emby',
  name: 'Home Emby',
  serverType: 'emby',
  serverUrl: 'http://emby.local:8096',
  apiKey: 'emby-key',
  allowInvalidCerts: false,
  createdAt: 1000,
  updatedAt: 1000,
};

const jellyfinProfile: MediaServerProfile = {
  id: 'jf-1',
  name: 'Remote Jellyfin',
  serverType: 'jellyfin',
  serverUrl: 'https://jellyfin.local:8920',
  apiKey: 'jf-key',
  allowInvalidCerts: true,
  createdAt: 2000,
  updatedAt: 2000,
};

describe('useEmbySync', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    useConfigStore.getState().setConfig({ activeMediaServerId: undefined });
    useToolsStore.setState({ embyServerName: null });
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null, isQueuePaused: true });
    useTranscriptStore.setState({ mode: 'tools', ingestionFocus: 'batch' });
    vi.mocked(embyService.getEmbyProcessingSettings).mockResolvedValue(null);
  });

  afterEach(() => {
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null, isQueuePaused: false });
    cleanup();
  });

  it('restores persisted activeMediaServerId on mount without calling saveEmbyConfig', async () => {
    useConfigStore.getState().setConfig({ activeMediaServerId: 'jf-1' });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([
      embyProfile,
      jellyfinProfile,
    ]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-jf-guid',
      serverName: 'Remote Jellyfin',
      version: '10.9.1',
      hasSubtitleManagerPlugin: true,
      serverType: 'jellyfin',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([
      { id: 'lib-jf', name: 'Movies', collectionType: 'movies' },
    ]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    expect(result.current.activeProfileId).toBe('jf-1');
    expect(result.current.activeProfile?.serverType).toBe('jellyfin');
    expect(mediaServerService.getMediaServerLibraries).toHaveBeenCalledWith('jf-1');
    expect(embyService.saveEmbyConfig).not.toHaveBeenCalled();
  });

  it('does not fall back to legacy Emby libraries when Jellyfin returns no libraries', async () => {
    useConfigStore.getState().setConfig({ activeMediaServerId: 'jf-1' });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([jellyfinProfile]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-jf-guid',
      serverName: 'Remote Jellyfin',
      version: '10.9.1',
      hasSubtitleManagerPlugin: true,
      serverType: 'jellyfin',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValue([
      { id: 'legacy-emby-library', name: 'Legacy Emby Library' },
    ]);

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    expect(result.current.libraries).toEqual([]);
    expect(embyService.getEmbyLibraries).not.toHaveBeenCalled();
  });

  it('keeps the legacy library fallback for the default Emby profile', async () => {
    useConfigStore.getState().setConfig({ activeMediaServerId: 'default-emby' });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([embyProfile]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-emby-guid',
      serverName: 'Home Emby',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
      serverType: 'emby',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValue([
      { id: 'legacy-library', name: 'Legacy Library' },
    ]);

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
    });

    expect(result.current.libraries).toEqual([{ id: 'legacy-library', name: 'Legacy Library' }]);
    expect(mediaServerService.getMediaServerLibraries).toHaveBeenCalledWith('default-emby');
    expect(embyService.getEmbyLibraries).toHaveBeenCalledTimes(1);
  });

  it('does not use legacy Emby connection checks for a Jellyfin profile', async () => {
    useConfigStore.getState().setConfig({ activeMediaServerId: 'jf-1' });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([jellyfinProfile]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockRejectedValue(
      new Error('Jellyfin connection failed')
    );
    vi.mocked(embyService.testEmbyConnection).mockResolvedValue({
      id: 'wrong-server',
      serverName: 'Legacy Emby',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
      serverType: 'emby',
    });

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.connectionError).toBe('Jellyfin connection failed');
    });

    expect(embyService.testEmbyConnection).not.toHaveBeenCalled();
  });

  it('selectProfile switches to Jellyfin without overwriting legacy Emby config', async () => {
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([
      embyProfile,
      jellyfinProfile,
    ]);
    vi.mocked(mediaServerService.setActiveMediaServerProfile).mockResolvedValue(undefined);
    vi.mocked(mediaServerService.testMediaServerConnection)
      .mockResolvedValueOnce({
        id: 'srv-emby-guid',
        serverName: 'Home Emby',
        version: '4.8.8.0',
        hasSubtitleManagerPlugin: true,
        serverType: 'emby',
      })
      .mockResolvedValueOnce({
        id: 'srv-jf-guid',
        serverName: 'Remote Jellyfin',
        version: '10.9.1',
        hasSubtitleManagerPlugin: true,
        serverType: 'jellyfin',
      });
    vi.mocked(mediaServerService.getMediaServerLibraries)
      .mockResolvedValueOnce([{ id: 'lib-emby', name: 'Emby Movies' }])
      .mockResolvedValueOnce([{ id: 'lib-jf', name: 'JF Movies' }]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.activeProfileId).toBe('default-emby');
      expect(result.current.isConnected).toBe(true);
    });

    await act(async () => {
      await result.current.selectProfile('jf-1');
    });

    expect(mediaServerService.setActiveMediaServerProfile).toHaveBeenCalledWith('jf-1');
    expect(result.current.activeProfileId).toBe('jf-1');
    expect(result.current.serverInfo?.serverName).toBe('Remote Jellyfin');
    expect(mediaServerService.getMediaServerLibraries).toHaveBeenLastCalledWith('jf-1');
    expect(embyService.saveEmbyConfig).not.toHaveBeenCalled();
  });

  it('does not fall back to legacy Emby items when a Jellyfin item request fails', async () => {
    useConfigStore.getState().setConfig({ activeMediaServerId: 'jf-1' });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([jellyfinProfile]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-jf-guid',
      serverName: 'Remote Jellyfin',
      version: '10.9.1',
      hasSubtitleManagerPlugin: true,
      serverType: 'jellyfin',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([
      { id: 'lib-jf', name: 'Movies' },
    ]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });
    vi.mocked(embyService.getEmbyItems).mockResolvedValue({
      totalRecordCount: 1,
      items: [
        {
          id: 'legacy-emby-item',
          name: 'Legacy Item',
          hasChineseSubtitles: false,
          hasEmbeddedChinese: false,
          mediaStreams: [],
        },
      ],
    });

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.isConnected).toBe(true);
      expect(mediaServerService.getMediaServerItems).toHaveBeenCalled();
    });
    vi.mocked(embyService.getEmbyItems).mockClear();
    vi.mocked(mediaServerService.getMediaServerItems).mockRejectedValueOnce(
      new Error('Jellyfin item request failed')
    );

    await act(async () => {
      await result.current.fetchItems();
    });

    expect(embyService.getEmbyItems).not.toHaveBeenCalled();
    expect(result.current.connectionError).toBe('Jellyfin item request failed');
  });

  it('handleLogin syncs new Emby profile before fetching libraries even when Jellyfin was active', async () => {
    useConfigStore.getState().setConfig({ activeMediaServerId: 'jf-1' });
    vi.mocked(mediaServerService.listMediaServerProfiles)
      .mockResolvedValueOnce([jellyfinProfile])
      .mockResolvedValueOnce([jellyfinProfile, embyProfile]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-jf-guid',
      serverName: 'Remote Jellyfin',
      version: '10.9.1',
      hasSubtitleManagerPlugin: true,
      serverType: 'jellyfin',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([
      { id: 'lib-1', name: 'Movies' },
    ]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });
    vi.mocked(embyService.loginEmby).mockResolvedValue({
      id: 'srv-emby-guid',
      serverName: 'Home Emby',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
      serverType: 'emby',
    });
    vi.mocked(embyService.getEmbyConfig).mockResolvedValue({
      serverUrl: embyProfile.serverUrl,
      apiKey: embyProfile.apiKey,
      allowInvalidCerts: false,
    });

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.activeProfileId).toBe('jf-1');
    });

    await act(async () => {
      await result.current.handleLogin({
        serverUrl: 'http://emby.local:8096',
        username: 'admin',
        password: 'pw',
        allowInvalidCerts: false,
      });
    });

    expect(result.current.activeProfileId).toBe('default-emby');
    expect(mediaServerService.getMediaServerLibraries).toHaveBeenLastCalledWith('default-emby');
  });

  it('handleSubmitBatch preserves audioTrackLanguage when asrLanguage override is auto', async () => {
    saveEmbyToolOverrides({ asrLanguage: 'auto' });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([embyProfile]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-emby-guid',
      serverName: 'Home Emby',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
      serverType: 'emby',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([
      { id: 'lib-1', name: 'Anime' },
    ]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 1,
      items: [
        {
          id: 'anime-1',
          name: 'Episode 1',
          itemType: 'Episode',
          hasChineseSubtitles: false,
          hasEmbeddedChinese: false,
          mediaStreams: [
            {
              index: 1,
              streamType: 'Audio',
              language: 'jpn',
              isExternal: false,
              isForced: false,
              isDefault: true,
            },
          ],
        },
      ],
    });

    const { result } = renderHook(() => useEmbySync());

    await waitFor(() => {
      expect(result.current.items).toHaveLength(1);
    });

    act(() => {
      result.current.toggleItemSelection('anime-1');
    });
    act(() => {
      result.current.handleSubmitBatch();
    });

    const queue = useBatchQueueStore.getState().queueItems;
    expect(queue).toHaveLength(1);
    expect(queue[0].mediaServerContext?.serverId).toBe('default-emby');
    expect(queue[0].mediaServerContext?.sourceLanguage).toBe('auto');
    expect(queue[0].mediaServerContext?.audioTrackLanguage).toBe('ja');
    expect(result.current.queuedItemIds.has('anime-1')).toBe(true);
  });

  it('persists selectProfile selection in useConfigStore across unmount and remount', async () => {
    useConfigStore.getState().setConfig({ activeMediaServerId: 'default-emby' });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([
      embyProfile,
      jellyfinProfile,
    ]);
    vi.mocked(mediaServerService.setActiveMediaServerProfile).mockResolvedValue(undefined);
    vi.mocked(mediaServerService.testMediaServerConnection)
      .mockResolvedValueOnce({
        id: 'srv-emby-guid',
        serverName: 'Home Emby',
        version: '4.8.8.0',
        hasSubtitleManagerPlugin: true,
        serverType: 'emby',
      })
      .mockResolvedValue({
        id: 'srv-jf-guid',
        serverName: 'Remote Jellyfin',
        version: '10.9.1',
        hasSubtitleManagerPlugin: true,
        serverType: 'jellyfin',
      });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([
      { id: 'lib-1', name: 'Movies' },
    ]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });

    const first = renderHook(() => useEmbySync());
    await waitFor(() => {
      expect(first.result.current.activeProfileId).toBe('default-emby');
    });

    await act(async () => {
      await first.result.current.selectProfile('jf-1');
    });
    expect(useConfigStore.getState().config.activeMediaServerId).toBe('jf-1');
    first.unmount();

    const remounted = renderHook(() => useEmbySync());
    await waitFor(() => {
      expect(remounted.result.current.isConnected).toBe(true);
    });
    expect(remounted.result.current.activeProfileId).toBe('jf-1');
  });

  it('calls saveEmbyConfig instead of overwriting named profile when handleConnect uses a different URL', async () => {
    const namedEmby: MediaServerProfile = {
      id: 'emby-work',
      name: 'Work Emby',
      serverType: 'emby',
      serverUrl: 'http://work.local:8096',
      apiKey: 'work-key',
      allowInvalidCerts: false,
      createdAt: 1000,
      updatedAt: 1000,
    };
    useConfigStore.getState().setConfig({ activeMediaServerId: 'emby-work' });
    vi.mocked(mediaServerService.listMediaServerProfiles)
      .mockResolvedValueOnce([namedEmby])
      .mockResolvedValueOnce([
        namedEmby,
        {
          ...embyProfile,
          serverUrl: 'http://home.local:8096',
          apiKey: 'home-key',
        },
      ]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-emby-guid',
      serverName: 'Emby',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
      serverType: 'emby',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([
      { id: 'lib-1', name: 'Movies' },
    ]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });
    vi.mocked(embyService.saveEmbyConfig).mockResolvedValue(undefined);

    const { result } = renderHook(() => useEmbySync());
    await waitFor(() => {
      expect(result.current.activeProfileId).toBe('emby-work');
    });

    await act(async () => {
      await result.current.handleConnect({
        serverUrl: 'http://home.local:8096',
        apiKey: 'home-key',
        allowInvalidCerts: false,
      });
    });

    expect(embyService.saveEmbyConfig).toHaveBeenCalledWith({
      serverUrl: 'http://home.local:8096',
      apiKey: 'home-key',
      allowInvalidCerts: false,
    });
    expect(mediaServerService.saveMediaServerProfile).not.toHaveBeenCalled();
  });

  it('stores overwriteExistingExternal=false on mediaServerContext when overwrite is disabled', async () => {
    saveEmbyToolOverrides({ overwriteExistingExternal: false });
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([embyProfile]);
    vi.mocked(mediaServerService.testMediaServerConnection).mockResolvedValue({
      id: 'srv-emby-guid',
      serverName: 'Home Emby',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
      serverType: 'emby',
    });
    vi.mocked(mediaServerService.getMediaServerLibraries).mockResolvedValue([
      { id: 'lib-1', name: 'Movies' },
    ]);
    vi.mocked(mediaServerService.getMediaServerItems).mockResolvedValue({
      totalRecordCount: 1,
      items: [
        {
          id: 'movie-1',
          name: 'Movie 1',
          itemType: 'Movie',
          hasChineseSubtitles: true,
          hasEmbeddedChinese: false,
          mediaStreams: [
            {
              index: 2,
              streamType: 'Subtitle',
              language: 'chi',
              isExternal: true,
              isForced: false,
              isDefault: false,
            },
          ],
        },
      ],
    });

    const { result } = renderHook(() => useEmbySync());
    await waitFor(() => {
      expect(result.current.items).toHaveLength(1);
    });

    act(() => {
      result.current.toggleItemSelection('movie-1');
    });
    act(() => {
      result.current.handleSubmitBatch();
    });

    const queue = useBatchQueueStore.getState().queueItems;
    expect(queue).toHaveLength(1);
    expect(queue[0].mediaServerContext?.overwriteExistingExternal).toBe(false);
    expect(queue[0].mediaServerContext?.existingExternalSubtitleIndex).toBeUndefined();
  });
});
