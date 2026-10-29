import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getEmbyConfig,
  getEmbyItems,
  getEmbyLibraries,
  saveEmbyConfig,
  testEmbyConnection,
} from '../../../../services/emby/embyService';
import { type AddEmbyItemEntry, useBatchQueueStore } from '../../../../stores/batchQueueStore';
import { useTranscriptStore } from '../../../../stores/transcriptStore';
import type { EmbySubtitleMode } from '../../../../types/batchQueue';
import type {
  EmbyConnectionConfig,
  EmbyLibrary,
  EmbyMediaItem,
  EmbyServerInfo,
} from '../../../../types/emby';
import { extractErrorMessage } from '../../../../utils/errorUtils';
import { findBestAudioStream, hasTargetSubtitle, normalizeLanguageCode } from './embyLanguageUtils';

function resolveInitialTargetLanguage(lang?: string): string {
  if (!lang) return 'zh-CN';
  const norm = normalizeLanguageCode(lang);
  if (['zh-CN', 'zh-TW', 'en', 'ja', 'ko'].includes(norm)) {
    return norm;
  }
  return 'zh-CN';
}

export interface UseEmbySyncReturn {
  // Connection
  config: EmbyConnectionConfig;
  setConfig: React.Dispatch<React.SetStateAction<EmbyConnectionConfig>>;
  serverInfo: EmbyServerInfo | null;
  isConnecting: boolean;
  connectionError: string | null;
  isConnected: boolean;
  handleConnect: (cfgToTest?: EmbyConnectionConfig) => Promise<void>;

  // Modals
  isConfigModalOpen: boolean;
  setIsConfigModalOpen: (open: boolean) => void;
  isPluginModalOpen: boolean;
  setIsPluginModalOpen: (open: boolean) => void;

  // Libraries & Items
  libraries: EmbyLibrary[];
  selectedLibraryId: string | null;
  setSelectedLibraryId: (id: string | null) => void;
  items: EmbyMediaItem[];
  filteredItems: EmbyMediaItem[];
  isLoadingItems: boolean;
  fetchItems: () => Promise<void>;

  // Search & Language Filtering
  searchTerm: string;
  setSearchTerm: (term: string) => void;
  onlyMissingTargetSubtitles: boolean;
  setOnlyMissingTargetSubtitles: (onlyMissing: boolean) => void;
  targetLanguage: string;
  setTargetLanguage: (lang: string) => void;

  // Grouping & Expansion
  groupedEpisodes: {
    groups: Record<string, EmbyMediaItem[]>;
    standalone: EmbyMediaItem[];
  };
  expandedSeasons: Set<string>;
  toggleSeasonExpansion: (key: string) => void;

  // Selection & Queue Deduplication
  selectedItemIds: Set<string>;
  queuedItemIds: Set<string>;
  toggleItemSelection: (id: string) => void;
  selectAllMissingInSeason: (seasonItems: EmbyMediaItem[]) => void;
  selectAllMissingMovies: (movies: EmbyMediaItem[]) => void;
  clearSelection: () => void;

  // Audio Tracks & Options
  audioTrackMap: Record<string, number>;
  handleAudioTrackChange: (itemId: string, streamIndex: number) => void;
  subtitleMode: EmbySubtitleMode;
  setSubtitleMode: (mode: EmbySubtitleMode) => void;
  overwriteExistingExternal: boolean;
  setOverwriteExistingExternal: (overwrite: boolean) => void;

  // Batch Submission
  handleSubmitBatch: () => void;
  submittedNotice: string | null;
}

export function useEmbySync(): UseEmbySyncReturn {
  const { t, i18n } = useTranslation();

  // Connection State
  const [serverInfo, setServerInfo] = useState<EmbyServerInfo | null>(null);
  const [config, setConfig] = useState<EmbyConnectionConfig>({
    serverUrl: '',
    apiKey: '',
    allowInvalidCerts: false,
  });
  const [isConnecting, setIsConnecting] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [isConfigModalOpen, setIsConfigModalOpen] = useState(false);
  const [isPluginModalOpen, setIsPluginModalOpen] = useState(false);

  // Library & Browse State
  const [libraries, setLibraries] = useState<EmbyLibrary[]>([]);
  const [selectedLibraryId, setSelectedLibraryId] = useState<string | null>(null);
  const [items, setItems] = useState<EmbyMediaItem[]>([]);
  const [isLoadingItems, setIsLoadingItems] = useState(false);

  // Filter & Search State
  const [searchTerm, setSearchTerm] = useState('');
  const [onlyMissingTargetSubtitles, setOnlyMissingTargetSubtitles] = useState(false);
  const [targetLanguage, setTargetLanguage] = useState<string>(() =>
    resolveInitialTargetLanguage(i18n.language)
  );

  // Selection & Track Map
  const [selectedItemIds, setSelectedItemIds] = useState<Set<string>>(new Set());
  const [audioTrackMap, setAudioTrackMap] = useState<Record<string, number>>({});
  const [expandedSeasons, setExpandedSeasons] = useState<Set<string>>(new Set());

  // Strategy Options
  const [subtitleMode, setSubtitleMode] = useState<EmbySubtitleMode>('bilingual');
  const [overwriteExistingExternal, setOverwriteExistingExternal] = useState(false);
  const [submittedNotice, setSubmittedNotice] = useState<string | null>(null);

  // Queue deduplication: find items currently pending or processing in batch queue
  const queueItems = useBatchQueueStore((state) => state.queueItems);
  const queuedItemIds = useMemo(() => {
    const set = new Set<string>();
    for (const item of queueItems) {
      if (item.embyContext?.itemId && (item.status === 'pending' || item.status === 'processing')) {
        set.add(item.embyContext.itemId);
      }
    }
    return set;
  }, [queueItems]);

  // Connect to Emby server
  const handleConnect = useCallback(
    async (cfgToTest?: EmbyConnectionConfig) => {
      const targetConfig = cfgToTest || config;
      if (!targetConfig.serverUrl.trim() || !targetConfig.apiKey.trim()) {
        setIsConfigModalOpen(true);
        return;
      }

      setIsConnecting(true);
      setConnectionError(null);
      try {
        const info = await testEmbyConnection(targetConfig);
        setServerInfo(info);
        await saveEmbyConfig(targetConfig);
        setIsConfigModalOpen(false);

        const libs = await getEmbyLibraries();
        setLibraries(libs);
        if (libs.length > 0 && !selectedLibraryId) {
          setSelectedLibraryId(libs[0].id);
        }
      } catch (err: unknown) {
        setConnectionError(extractErrorMessage(err) || String(err));
        setServerInfo(null);
      } finally {
        setIsConnecting(false);
      }
    },
    [config, selectedLibraryId]
  );

  // Load initial saved config on mount
  useEffect(() => {
    async function loadConfig() {
      try {
        const saved = await getEmbyConfig();
        if (saved?.serverUrl && saved.apiKey) {
          setConfig(saved);
          handleConnect(saved);
        }
      } catch (err) {
        console.error('Failed to load Emby config:', err);
      }
    }
    loadConfig();
  }, [handleConnect]);

  // Fetch items for selected library and search term
  const fetchItems = useCallback(async () => {
    if (!serverInfo) return;
    setIsLoadingItems(true);
    try {
      const res = await getEmbyItems({
        parentId: selectedLibraryId || undefined,
        searchTerm: searchTerm.trim() || undefined,
        startIndex: 0,
        limit: 300,
      });
      setItems(res.items);

      // Initialize default audio tracks
      const newMap: Record<string, number> = {};
      const seasons = new Set<string>();
      for (const item of res.items) {
        const bestAudio = findBestAudioStream(item.mediaStreams);
        newMap[item.id] = bestAudio ? bestAudio.index : 0;
        if (item.seriesName || item.seasonName) {
          const seasonKey = `${item.seriesName || ''} - ${item.seasonName || 'Season 1'}`;
          seasons.add(seasonKey);
        }
      }
      setAudioTrackMap(newMap);
      setExpandedSeasons(seasons);
    } catch (err) {
      console.error('Failed to fetch items:', err);
    } finally {
      setIsLoadingItems(false);
    }
  }, [serverInfo, selectedLibraryId, searchTerm]);

  useEffect(() => {
    if (serverInfo) {
      fetchItems();
    }
  }, [serverInfo, fetchItems]);

  // Filter items based on search term and target subtitle presence
  const filteredItems = useMemo(() => {
    let result = items;
    if (searchTerm.trim()) {
      const term = searchTerm.trim().toLowerCase();
      result = result.filter(
        (item) =>
          item.name.toLowerCase().includes(term) || item.seriesName?.toLowerCase().includes(term)
      );
    }
    if (onlyMissingTargetSubtitles) {
      result = result.filter((item) => !hasTargetSubtitle(item.mediaStreams, targetLanguage));
    }
    return result;
  }, [items, searchTerm, onlyMissingTargetSubtitles, targetLanguage]);

  // Group items by season for TV episodes or standalone for movies
  const groupedEpisodes = useMemo(() => {
    const groups: Record<string, EmbyMediaItem[]> = {};
    const standalone: EmbyMediaItem[] = [];

    for (const item of filteredItems) {
      if (item.seriesName || item.itemType?.toLowerCase() === 'episode') {
        const key = `${item.seriesName || 'Series'} - ${item.seasonName || 'Season'}`;
        if (!groups[key]) groups[key] = [];
        groups[key].push(item);
      } else {
        standalone.push(item);
      }
    }

    // Sort episodes inside each season group
    for (const key of Object.keys(groups)) {
      groups[key].sort((a, b) => (a.episodeIndex ?? 0) - (b.episodeIndex ?? 0));
    }

    return { groups, standalone };
  }, [filteredItems]);

  // Selection actions with queue lockout protection
  const toggleItemSelection = useCallback(
    (id: string) => {
      if (queuedItemIds.has(id)) return;
      setSelectedItemIds((prev) => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      });
    },
    [queuedItemIds]
  );

  const selectAllMissingInSeason = useCallback(
    (seasonItems: EmbyMediaItem[]) => {
      setSelectedItemIds((prev) => {
        const next = new Set(prev);
        for (const item of seasonItems) {
          if (
            !hasTargetSubtitle(item.mediaStreams, targetLanguage) &&
            !queuedItemIds.has(item.id)
          ) {
            next.add(item.id);
          }
        }
        return next;
      });
    },
    [targetLanguage, queuedItemIds]
  );

  const selectAllMissingMovies = useCallback(
    (movies: EmbyMediaItem[]) => {
      setSelectedItemIds((prev) => {
        const next = new Set(prev);
        for (const item of movies) {
          if (
            !hasTargetSubtitle(item.mediaStreams, targetLanguage) &&
            !queuedItemIds.has(item.id)
          ) {
            next.add(item.id);
          }
        }
        return next;
      });
    },
    [targetLanguage, queuedItemIds]
  );

  const clearSelection = useCallback(() => {
    setSelectedItemIds(new Set());
  }, []);

  const toggleSeasonExpansion = useCallback((key: string) => {
    setExpandedSeasons((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const handleAudioTrackChange = useCallback((itemId: string, streamIndex: number) => {
    setAudioTrackMap((prev) => ({ ...prev, [itemId]: streamIndex }));
  }, []);

  // Batch submission
  const handleSubmitBatch = useCallback(() => {
    if (selectedItemIds.size === 0 || !serverInfo) return;

    const entries: AddEmbyItemEntry[] = [];
    for (const id of selectedItemIds) {
      if (queuedItemIds.has(id)) continue;
      const item = items.find((it) => it.id === id);
      if (!item) continue;

      const bestAudio = findBestAudioStream(item.mediaStreams);
      const audioStreamIndex = audioTrackMap[item.id] ?? bestAudio?.index ?? 0;
      const selectedAudio =
        item.mediaStreams.find((s) => s.index === audioStreamIndex) ?? bestAudio;

      let sourceLanguage: string | undefined;
      const rawLang = selectedAudio?.language?.trim().toLowerCase();
      if (rawLang && rawLang !== 'und') {
        const norm = normalizeLanguageCode(rawLang);
        sourceLanguage = norm || undefined;
      }

      let existingExternalSubtitleIndex: number | undefined;
      if (overwriteExistingExternal) {
        const extSub = item.mediaStreams.find(
          (s) => s.streamType.toLowerCase() === 'subtitle' && s.isExternal
        );
        if (extSub) {
          existingExternalSubtitleIndex = extSub.index;
        }
      }

      const displayName = item.seriesName
        ? `${item.seriesName} ${item.seasonName || ''} E${item.episodeIndex ?? ''} - ${item.name}`
        : item.name;

      entries.push({
        itemId: item.id,
        itemName: displayName,
        audioStreamIndex,
        subtitleMode,
        targetLanguage,
        sourceLanguage,
        existingExternalSubtitleIndex,
        serverId: serverInfo.id,
        serverUrl: config.serverUrl,
      });
    }

    if (entries.length === 0) return;

    useBatchQueueStore.getState().addEmbyItems({ items: entries });
    setSelectedItemIds(new Set());
    setSubmittedNotice(
      t('emby.submitted_success', {
        count: entries.length,
        defaultValue: `Successfully added ${entries.length} tasks to the transcription queue`,
      })
    );
    setTimeout(() => setSubmittedNotice(null), 4000);

    // Redirect to batch queue view
    useTranscriptStore.getState().setMode('create', 'batch');
  }, [
    selectedItemIds,
    queuedItemIds,
    serverInfo,
    items,
    audioTrackMap,
    overwriteExistingExternal,
    subtitleMode,
    targetLanguage,
    config.serverUrl,
    t,
  ]);

  return {
    config,
    setConfig,
    serverInfo,
    isConnecting,
    connectionError,
    isConnected: Boolean(serverInfo),
    handleConnect,

    isConfigModalOpen,
    setIsConfigModalOpen,
    isPluginModalOpen,
    setIsPluginModalOpen,

    libraries,
    selectedLibraryId,
    setSelectedLibraryId,
    items,
    filteredItems,
    isLoadingItems,
    fetchItems,

    searchTerm,
    setSearchTerm,
    onlyMissingTargetSubtitles,
    setOnlyMissingTargetSubtitles,
    targetLanguage,
    setTargetLanguage,

    groupedEpisodes,
    expandedSeasons,
    toggleSeasonExpansion,

    selectedItemIds,
    queuedItemIds,
    toggleItemSelection,
    selectAllMissingInSeason,
    selectAllMissingMovies,
    clearSelection,

    audioTrackMap,
    handleAudioTrackChange,
    subtitleMode,
    setSubtitleMode,
    overwriteExistingExternal,
    setOverwriteExistingExternal,

    handleSubmitBatch,
    submittedNotice,
  };
}
