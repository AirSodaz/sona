import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as embyService from '../../../../services/emby/embyService';
import { useBatchQueueStore } from '../../../../stores/batchQueueStore';
import type { EmbyItemListResult, EmbyLibrary, EmbyServerInfo } from '../../../../types/emby';
import { ToolEmbySyncPanel } from '../ToolEmbySyncPanel';

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

vi.mock('../../../../services/emby/embyService', () => ({
  getEmbyConfig: vi.fn(),
  saveEmbyConfig: vi.fn(),
  testEmbyConnection: vi.fn(),
  getEmbyLibraries: vi.fn(),
  getEmbyItems: vi.fn(),
  downloadEmbyAudio: vi.fn(),
  uploadEmbySubtitle: vi.fn(),
}));
vi.mock('../../../../services/recoveryService', () => ({
  persistQueueRecoverySnapshot: vi.fn(),
  toBatchQueueItem: vi.fn((x) => x),
}));

const mockServerInfo: EmbyServerInfo = {
  id: 'server_123',
  serverName: 'My Emby Media',
  version: '4.8.8.0',
  hasSubtitleManagerPlugin: false, // Testing plugin warning display
};

const mockLibraries: EmbyLibrary[] = [
  { id: 'lib_movies', name: '电影', collectionType: 'movies' },
  { id: 'lib_shows', name: '电视剧', collectionType: 'tvshows' },
];

const mockItemsResult: EmbyItemListResult = {
  totalRecordCount: 3,
  items: [
    {
      id: 'movie_1',
      name: '盗梦空间 Inception',
      itemType: 'Movie',
      hasChineseSubtitles: true,
      hasEmbeddedChinese: true,
      runtimeTicks: 88800000000,
      mediaStreams: [
        {
          index: 0,
          streamType: 'Video',
          codec: 'h264',
          isExternal: false,
          isForced: false,
          isDefault: true,
        },
        {
          index: 1,
          streamType: 'Audio',
          language: 'eng',
          displayTitle: 'English 5.1',
          codec: 'ac3',
          isExternal: false,
          isForced: false,
          isDefault: true,
        },
        {
          index: 2,
          streamType: 'Subtitle',
          language: 'chi',
          displayTitle: '官方简中',
          isExternal: false,
          isForced: false,
          isDefault: false,
        },
      ],
    },
    {
      id: 'ep_1',
      name: '绝命毒师 Pilot',
      seriesName: '绝命毒师',
      seasonName: 'Season 1',
      seasonIndex: 1,
      episodeIndex: 1,
      itemType: 'Episode',
      hasChineseSubtitles: false,
      hasEmbeddedChinese: false,
      runtimeTicks: 34800000000,
      mediaStreams: [
        {
          index: 1,
          streamType: 'Audio',
          language: 'eng',
          displayTitle: 'English Stereo',
          isExternal: false,
          isForced: false,
          isDefault: true,
        },
      ],
    },
    {
      id: 'ep_2',
      name: '绝命毒师 Cat in the Bag',
      seriesName: '绝命毒师',
      seasonName: 'Season 1',
      seasonIndex: 1,
      episodeIndex: 2,
      itemType: 'Episode',
      hasChineseSubtitles: true,
      hasEmbeddedChinese: false,
      runtimeTicks: 28800000000,
      mediaStreams: [
        {
          index: 1,
          streamType: 'Audio',
          language: 'eng',
          displayTitle: 'English Stereo',
          isExternal: false,
          isForced: false,
          isDefault: true,
        },
        {
          index: 2,
          streamType: 'Subtitle',
          language: 'chi',
          displayTitle: '外挂中字',
          isExternal: true,
          isForced: false,
          isDefault: true,
        },
      ],
    },
  ],
};

describe('ToolEmbySyncPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null });

    vi.mocked(embyService.getEmbyConfig).mockResolvedValue({
      serverUrl: 'http://192.168.1.100:8096',
      apiKey: 'test_api_key',
      allowInvalidCerts: true,
    });
    vi.mocked(embyService.testEmbyConnection).mockResolvedValue(mockServerInfo);
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValue(mockLibraries);
    vi.mocked(embyService.getEmbyItems).mockResolvedValue(mockItemsResult);
  });

  afterEach(() => {
    cleanup();
  });

  it('connects to Emby on load and displays server information and plugin warning', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('My Emby Media (v4.8.8.0)')).toBeDefined();
    });

    // Check plugin missing banner because hasSubtitleManagerPlugin = false
    expect(screen.getByText(/Server plugin Emby.SubtitleManager is not detected/i)).toBeDefined();
    expect(screen.getByText('Plugin Guide')).toBeDefined();
  });

  it('filters items when checking "仅看缺失中文字幕"', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
      expect(screen.getByText(/E1 - 绝命毒师 Pilot/)).toBeDefined();
    });

    const filterCheckbox = screen.getByLabelText('Only missing Chinese subtitles');
    fireEvent.click(filterCheckbox);

    // Inception has Chinese subtitles, so it should be filtered out
    expect(screen.queryByText('盗梦空间 Inception')).toBeNull();
    // Episode 1 lacks Chinese subtitles, so it should remain
    expect(screen.getByText(/E1 - 绝命毒师 Pilot/)).toBeDefined();
    // Episode 2 has external Chinese subtitles, so it should be filtered out
    expect(screen.queryByText(/E2 - 绝命毒师 Cat in the Bag/)).toBeNull();
  });

  it('supports season accordion expansion and selecting all missing episodes', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText(/绝命毒师 - Season 1/)).toBeDefined();
    });

    // Click "Select Missing Episodes in Season"
    const selectAllBtn = screen.getByText('Select Missing Episodes in Season');
    fireEvent.click(selectAllBtn);

    // Only Episode 1 is missing, so submit button should show 1 selected item
    expect(screen.getByText(/Transcribe and Sync \(1 selected\)/)).toBeDefined();
  });

  it('submits selected items to batchQueueStore with complete embyContext', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
    });

    // Select Inception and Episode 1
    const checkboxes = screen.getAllByRole('checkbox');
    // First checkbox is filter, second is Inception, third is Episode 1, fourth is Episode 2, fifth is overwrite
    fireEvent.click(checkboxes[1]); // Inception

    expect(screen.getByText(/Transcribe and Sync \(1 selected\)/)).toBeDefined();

    // Click submit
    const submitBtn = screen.getByText(/Transcribe and Sync \(1 selected\)/);
    fireEvent.click(submitBtn);

    // Verify batchQueueStore contains the enqueued item
    const queue = useBatchQueueStore.getState().queueItems;
    expect(queue.length).toBe(1);
    expect(queue[0].filename).toBe('盗梦空间 Inception');
    expect(queue[0].embyContext).toBeDefined();
    expect(queue[0].embyContext?.itemId).toBe('movie_1');
    expect(queue[0].embyContext?.serverId).toBe('server_123');
    expect(queue[0].embyContext?.serverUrl).toBe('http://192.168.1.100:8096');
    expect(queue[0].embyContext?.audioStreamIndex).toBe(1); // Audio track index
    expect(queue[0].embyContext?.subtitleMode).toBe('bilingual');
    expect(queue[0].embyContext?.targetLanguage).toBe('zh-CN');
  });
});
