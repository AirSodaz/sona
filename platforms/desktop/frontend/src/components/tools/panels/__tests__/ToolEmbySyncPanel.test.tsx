import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as embyService from '../../../../services/emby/embyService';
import { saveEmbyToolOverrides } from '../../../../services/emby/embySettingsService';
import { useBatchQueueStore } from '../../../../stores/batchQueueStore';
import { useToolsStore } from '../../../../stores/toolsStore';
import { useTranscriptStore } from '../../../../stores/transcriptStore';
import type {
  EmbyItemListResult,
  EmbyLibrary,
  EmbyServerInfo,
  EmbyToolOverrides,
} from '../../../../types/emby';
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
  loginEmby: vi.fn(),
  getEmbyLibraries: vi.fn(),
  getEmbyItems: vi.fn(),
  downloadEmbyAudio: vi.fn(),
  uploadEmbySubtitle: vi.fn(),
  getEmbyProcessingSettings: vi.fn(),
  saveEmbyProcessingSettings: vi.fn(),
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
    localStorage.clear();
    useToolsStore.setState({ embyServerName: null });
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null });
    useTranscriptStore.setState({ mode: 'tools', ingestionFocus: 'batch' });
    vi.mocked(embyService.getEmbyConfig).mockResolvedValue({
      serverUrl: 'http://192.168.1.100:8096',
      apiKey: 'test_api_key',
      allowInvalidCerts: true,
    });
    vi.mocked(embyService.testEmbyConnection).mockResolvedValue(mockServerInfo);
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValue(mockLibraries);
    vi.mocked(embyService.getEmbyItems).mockResolvedValue(mockItemsResult);
    vi.mocked(embyService.getEmbyProcessingSettings).mockResolvedValue(null);
    vi.mocked(embyService.saveEmbyProcessingSettings).mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
    localStorage.clear();
    useToolsStore.setState({ embyServerName: null });
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

  it('filters items when checking only missing target subtitles', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
      expect(screen.getByText(/E1 - 绝命毒师 Pilot/)).toBeDefined();
    });

    const filterCheckbox = screen.getByLabelText(/Only show items missing/i);
    fireEvent.click(filterCheckbox);

    // Inception has Chinese subtitles, so it should be filtered out
    expect(screen.queryByText('盗梦空间 Inception')).toBeNull();
    // Episode 1 lacks Chinese subtitles, so it should remain
    expect(screen.getByText(/E1 - 绝命毒师 Pilot/)).toBeDefined();
    // Episode 2 has external Chinese subtitles, so it should be filtered out
    expect(screen.queryByText(/E2 - 绝命毒师 Cat in the Bag/)).toBeNull();
  });

  it('treats embedded and external subtitles equally as present (binary check)', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
      expect(screen.getByText(/E1 - 绝命毒师 Pilot/)).toBeDefined();
      expect(screen.getByText(/E2 - 绝命毒师 Cat in the Bag/)).toBeDefined();
    });

    // Inception has embedded Chinese -> Has Subtitles
    // Episode 2 has external Chinese -> Has Subtitles
    const presentTags = screen.getAllByText('Has Subtitles');
    expect(presentTags.length).toBe(2);

    // Episode 1 has no subtitle -> Missing Subtitles
    const missingTags = screen.getAllByText('Missing Subtitles');
    expect(missingTags.length).toBe(1);
  });

  it('dynamically updates subtitle presence when target language is changed', async () => {
    const customItemsResult: EmbyItemListResult = {
      totalRecordCount: 1,
      items: [
        {
          id: 'movie_ja',
          name: '千与千寻 Spirited Away',
          itemType: 'Movie',
          hasChineseSubtitles: false,
          hasEmbeddedChinese: false,
          mediaStreams: [
            {
              index: 0,
              streamType: 'Audio',
              language: 'jpn',
              isExternal: false,
              isForced: false,
              isDefault: true,
            },
            {
              index: 1,
              streamType: 'Subtitle',
              language: 'jpn',
              displayTitle: 'Japanese',
              isExternal: false,
              isForced: false,
              isDefault: false,
            },
          ],
        },
      ],
    };

    vi.mocked(embyService.getEmbyItems).mockResolvedValue(customItemsResult);

    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('千与千寻 Spirited Away')).toBeDefined();
    });

    // Initially target language is zh-CN, so Japanese subtitle is missing
    expect(screen.getByText('Missing Subtitles')).toBeDefined();
    expect(screen.queryByText('Has Subtitles')).toBeNull();

    // Change target language to Japanese (ja)
    const targetLangBtn = screen.getByRole('button', { name: /Target Language:/i });
    fireEvent.click(targetLangBtn);
    fireEvent.click(screen.getByRole('option', { name: /日本語/i }));
    // Now it matches the Japanese subtitle!
    expect(screen.getByText('Has Subtitles')).toBeDefined();
    expect(screen.queryByText('Missing Subtitles')).toBeNull();
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
    expect(screen.getByText(/Transcribe \(1\)/)).toBeDefined();
  });

  it('submits selected items to batchQueueStore with complete embyContext and navigates to batch queue', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
    });

    // Select Inception
    const inceptionCheckbox = screen.getByRole('checkbox', {
      name: /Select 盗梦空间 Inception/i,
    });
    fireEvent.click(inceptionCheckbox);

    expect(screen.getByText(/Transcribe \(1\)/)).toBeDefined();

    // Click submit
    const submitBtn = screen.getByRole('button', { name: /Transcribe \(1\)/ });
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
    expect(queue[0].embyContext?.sourceLanguage).toBe('auto'); // Default strict auto recognition
    expect(queue[0].resolvedConfigSnapshot?.language).toBe('auto');
    expect(queue[0].embyContext?.subtitleMode).toBe('bilingual');
    expect(queue[0].embyContext?.targetLanguage).toBe('zh-CN');

    // Verify navigation triggered: transcript mode set to 'create', focus to 'batch'
    const transcriptState = useTranscriptStore.getState();
    expect(transcriptState.mode).toBe('create');
    expect(transcriptState.ingestionFocus).toBe('batch');
  });

  it('locks out items already pending or processing in batchQueueStore', async () => {
    // Pre-populate queue with ep_1 as pending
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'q-item-1',
          filename: '绝命毒师 Season 1 E1 - 绝命毒师 Pilot',
          filePath: '',
          status: 'pending',
          progress: 0,
          projectId: null,
          segments: [],
          embyContext: {
            serverId: 'server_123',
            serverUrl: 'http://192.168.1.100:8096',
            itemId: 'ep_1',
            itemName: '绝命毒师 Season 1 E1 - 绝命毒师 Pilot',
            audioStreamIndex: 1,
            subtitleMode: 'bilingual',
            targetLanguage: 'zh-CN',
          },
        },
      ],
      activeItemId: null,
    });
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText(/绝命毒师 - Season 1/)).toBeDefined();
    });

    // ep_1 row displays "In Queue"
    expect(screen.getByText('In Queue')).toBeDefined();
    // Checkbox for ep_1 is disabled
    const ep1Checkbox = screen.getByRole('checkbox', { name: /Select (?:E1 - )?绝命毒师 Pilot/i });
    expect(ep1Checkbox.hasAttribute('disabled')).toBe(true);

    // Clicking "Select Missing Episodes in Season" should not select ep_1 because it is in queue
    const selectAllBtn = screen.getByText('Select Missing Episodes in Season');
    fireEvent.click(selectAllBtn);

    // No new items selected
    expect(screen.queryByText(/Transcribe \(1\)/)).toBeNull();
  });

  it('renders refresh button without native title tooltip attribute', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
    });

    const refreshBtn = screen.getByRole('button', { name: /Refresh/i });
    expect(refreshBtn).toBeDefined();
    expect(refreshBtn.getAttribute('title')).toBeNull();
  });

  it('handles master checkbox select-all and toggle behavior in the toolbar', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
    });

    // Master checkbox in toolbar
    const masterCheckbox = screen.getByRole('checkbox', {
      name: /Select All/i,
    }) as HTMLInputElement;
    expect(masterCheckbox).toBeDefined();
    expect(masterCheckbox.checked).toBe(false);
    expect(masterCheckbox.indeterminate).toBe(false);

    // Select one item individually -> master checkbox should become indeterminate
    const inceptionCheckbox = screen.getByRole('checkbox', {
      name: /Select 盗梦空间 Inception/i,
    });
    fireEvent.click(inceptionCheckbox);
    expect(masterCheckbox.indeterminate).toBe(true);
    expect(screen.getByText(/Transcribe \(1\)/)).toBeDefined();

    // Clicking indeterminate master checkbox selects all selectable filtered items (3 total in mock)
    fireEvent.click(masterCheckbox);
    expect(masterCheckbox.checked).toBe(true);
    expect(masterCheckbox.indeterminate).toBe(false);
    expect(screen.getByText(/Transcribe \(3\)/)).toBeDefined();

    // Clicking checked master checkbox clears selection
    fireEvent.click(masterCheckbox);
    expect(masterCheckbox.checked).toBe(false);
    expect(masterCheckbox.indeterminate).toBe(false);
    expect(screen.queryByText(/Transcribe \(3\)/)).toBeNull();
    const submitBtn = screen.getByRole('button', { name: /Transcribe \(0\)/ });
    expect(submitBtn.hasAttribute('disabled')).toBe(true);
  });

  it('toggles season accordion collapse and expand on header click', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
      expect(screen.getByText(/E1 - 绝命毒师 Pilot/)).toBeDefined();
    });

    const seasonHeader = screen.getByText(/绝命毒师 - Season 1/);
    fireEvent.click(seasonHeader);
    expect(screen.queryByText(/E1 - 绝命毒师 Pilot/)).toBeNull();

    fireEvent.click(seasonHeader);
    expect(screen.getByText(/E1 - 绝命毒师 Pilot/)).toBeDefined();
  });

  it('opens server config modal and plugin guide modal when clicked', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
    });

    const serverSettingsBtn = screen.getByRole('button', { name: /Server Settings/i });
    fireEvent.click(serverSettingsBtn);
    expect(screen.getByText('Configure Emby Server Connection')).toBeDefined();

    const cancelBtn = screen.getByRole('button', { name: /Cancel/i });
    fireEvent.click(cancelBtn);

    const pluginGuideBtn = screen.getByRole('button', { name: /Plugin Guide/i });
    fireEvent.click(pluginGuideBtn);
    expect(screen.getByText('Install Emby.SubtitleManager Plugin')).toBeDefined();
  });
  it('opens processing settings modal, saves overrides, and enqueues items with effective snapshot', async () => {
    render(<ToolEmbySyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('盗梦空间 Inception')).toBeDefined();
    });

    const customOverrides: EmbyToolOverrides = {
      asrSelection: {
        engine: 'online',
        mode: 'batch',
        providerId: 'volcengine-doubao',
        modelId: 'doubao-pro',
        modelPath: '',
      },
      asrLanguage: 'ja',
      translationModelId: 'deepseek-v3',
      defaultSubtitleMode: 'pure_translation',
      overwriteExistingExternal: false,
    };
    saveEmbyToolOverrides(customOverrides);

    const processingSettingsBtn = screen.getByRole('button', { name: /Processing Settings/i });
    fireEvent.click(processingSettingsBtn);
    expect(screen.getByText('Emby Processing Settings')).toBeDefined();

    const saveBtn = screen.getByRole('button', { name: 'Save' });
    fireEvent.click(saveBtn);

    // Select Inception and submit
    const inceptionCheckbox = screen.getByRole('checkbox', {
      name: /Select 盗梦空间 Inception/i,
    });
    fireEvent.click(inceptionCheckbox);

    const submitBtn = screen.getByRole('button', { name: /Transcribe \(1\)/ });
    fireEvent.click(submitBtn);

    const queue = useBatchQueueStore.getState().queueItems;
    expect(queue.length).toBe(1);
    expect(queue[0].embyContext?.sourceLanguage).toBe('ja');
    expect(queue[0].embyContext?.subtitleMode).toBe('pure_translation');
    expect(queue[0].resolvedConfigSnapshot?.language).toBe('ja');
    expect(queue[0].resolvedConfigSnapshot?.asr.selections.batch).toEqual(
      customOverrides.asrSelection
    );
    expect(queue[0].resolvedConfigSnapshot?.llmSettings?.selections.translationModelId).toBe(
      'deepseek-v3'
    );
  });
});
