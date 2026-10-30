import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
import {
  findBestAudioStream,
  hasTargetSubtitle,
  matchSubtitleLanguage,
  normalizeLanguageCode,
} from './embyLanguageUtils';

export function getSeasonGroupKey(item: EmbyMediaItem): string {
  const series = item.seriesName || 'Series';
  const season =
    item.seasonName ||
    (item.seasonIndex !== undefined && item.seasonIndex !== null
      ? `Season ${item.seasonIndex}`
      : 'Season 1');
  return `${series} - ${season}`;
}

function resolveInitialTargetLanguage(lang?: string): string {
  if (!lang) return 'zh-CN';
  const norm = normalizeLanguageCode(lang);
  if (['zh-CN', 'zh-TW', 'en', 'ja', 'ko'].includes(norm)) {
    return norm;
  }
  const primary = lang.toLowerCase().split(/[-_]/)[0];
  const primaryNorm = normalizeLanguageCode(primary);
  if (['zh-CN', 'zh-TW', 'en', 'ja', 'ko'].includes(primaryNorm)) {
    return primaryNorm;
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
  selectAllFiltered: (items: EmbyMediaItem[]) => void;
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

  const configRef = useRef(config);
  configRef.current = config;
  const noticeTimeoutRef = useRef<number | null>(null);
  const hasLoadedRef = useRef(false);
  const fetchSeqRef = useRef(0);

  useEffect(() => {
    return () => {
      clearTimeout(noticeTimeoutRef.current ?? undefined);
    };
  }, []);

  // Clear selected items and expanded seasons on library or search change
  useEffect(() => {
    void selectedLibraryId;
    void searchTerm;
    setSelectedItemIds(new Set());
    setExpandedSeasons(new Set());
  }, [selectedLibraryId, searchTerm]);

  // Connect to Emby server
  const handleConnect = useCallback(async (cfgToTest?: EmbyConnectionConfig) => {
    const targetConfig = cfgToTest || configRef.current;
    if (!targetConfig.serverUrl.trim() || !targetConfig.apiKey.trim()) {
      setIsConfigModalOpen(true);
      return;
    }

    setIsConnecting(true);
    setConnectionError(null);
    try {
      const info = await testEmbyConnection(targetConfig);
      await saveEmbyConfig(targetConfig);
      setConfig(targetConfig);
      setIsConfigModalOpen(false);

      const libs = await getEmbyLibraries();
      setLibraries(libs);
      if (libs.length > 0) {
        setSelectedLibraryId((prev) => {
          if (prev && libs.some((lib) => lib.id === prev)) {
            return prev;
          }
          return libs[0].id;
        });
      } else {
        setSelectedLibraryId(null);
      }
      setServerInfo(info);
    } catch (err: unknown) {
      setConnectionError(extractErrorMessage(err) || String(err));
      setServerInfo(null);
    } finally {
      setIsConnecting(false);
    }
  }, []);

  // Load initial saved config on mount
  useEffect(() => {
    if (hasLoadedRef.current) return;
    hasLoadedRef.current = true;

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

  // Fetch items for selected library and search term with debounce and race cancellation
  useEffect(() => {
    if (!serverInfo) return;

    let cancelled = false;
    const timer = setTimeout(async () => {
      const seq = ++fetchSeqRef.current;
      setIsLoadingItems(true);
      try {
        const res = await getEmbyItems({
          parentId: selectedLibraryId || undefined,
          searchTerm: searchTerm.trim() || undefined,
          startIndex: 0,
          limit: 300,
        });
        if (cancelled || seq !== fetchSeqRef.current) return;
        setItems(res.items);

        setAudioTrackMap((prev) => {
          const next = { ...prev };
          for (const item of res.items) {
            if (next[item.id] === undefined) {
              const bestAudio = findBestAudioStream(item.mediaStreams);
              next[item.id] = bestAudio ? bestAudio.index : 0;
            }
          }
          return next;
        });

        setExpandedSeasons((prev) => {
          if (prev.size > 0) return prev;
          const seasons = new Set<string>();
          for (const item of res.items) {
            if (item.seriesName || item.itemType?.toLowerCase() === 'episode') {
              seasons.add(getSeasonGroupKey(item));
            }
          }
          return seasons;
        });
      } catch (err) {
        if (!cancelled && seq === fetchSeqRef.current) {
          console.error('Failed to fetch items:', err);
        }
      } finally {
        if (!cancelled && seq === fetchSeqRef.current) {
          setIsLoadingItems(false);
        }
      }
    }, 300);

    return () => {
      cancelled = true;
      fetchSeqRef.current++;
      clearTimeout(timer);
    };
  }, [serverInfo, selectedLibraryId, searchTerm]);

  const fetchItems = useCallback(async () => {
    if (!serverInfo) return;
    const seq = ++fetchSeqRef.current;
    setIsLoadingItems(true);
    try {
      const res = await getEmbyItems({
        parentId: selectedLibraryId || undefined,
        searchTerm: searchTerm.trim() || undefined,
        startIndex: 0,
        limit: 300,
      });
      if (seq !== fetchSeqRef.current) return;
      setItems(res.items);

      setAudioTrackMap((prev) => {
        const next = { ...prev };
        for (const item of res.items) {
          if (next[item.id] === undefined) {
            const bestAudio = findBestAudioStream(item.mediaStreams);
            next[item.id] = bestAudio ? bestAudio.index : 0;
          }
        }
        return next;
      });

      setExpandedSeasons((prev) => {
        if (prev.size > 0) return prev;
        const seasons = new Set<string>();
        for (const item of res.items) {
          if (item.seriesName || item.itemType?.toLowerCase() === 'episode') {
            seasons.add(getSeasonGroupKey(item));
          }
        }
        return seasons;
      });
    } catch (err) {
      if (seq === fetchSeqRef.current) {
        console.error('Failed to fetch items:', err);
      }
    } finally {
      if (seq === fetchSeqRef.current) {
        setIsLoadingItems(false);
      }
    }
  }, [serverInfo, selectedLibraryId, searchTerm]);

  const filteredItems = useMemo(() => {
    let result = items;
    if (onlyMissingTargetSubtitles) {
      result = result.filter((item) => !hasTargetSubtitle(item.mediaStreams, targetLanguage));
    }
    return result;
  }, [items, onlyMissingTargetSubtitles, targetLanguage]);

  // Group items by season for TV episodes or standalone for movies
  const groupedEpisodes = useMemo(() => {
    const groups: Record<string, EmbyMediaItem[]> = {};
    const standalone: EmbyMediaItem[] = [];

    for (const item of filteredItems) {
      if (item.seriesName || item.itemType?.toLowerCase() === 'episode') {
        const key = getSeasonGroupKey(item);
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
      setSelectedItemIds((prev) => {
        const next = new Set(prev);
        if (next.has(id)) {
          next.delete(id);
        } else if (!queuedItemIds.has(id)) {
          next.add(id);
        }
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

  const selectAllFiltered = useCallback(
    (itemsToSelect: EmbyMediaItem[]) => {
      setSelectedItemIds((prev) => {
        const next = new Set(prev);
        for (const item of itemsToSelect) {
          if (!queuedItemIds.has(item.id)) {
            next.add(item.id);
          }
        }
        return next;
      });
    },
    [queuedItemIds]
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
          (s) =>
            s.streamType.toLowerCase() === 'subtitle' &&
            s.isExternal &&
            matchSubtitleLanguage(s.language, s.displayTitle, targetLanguage)
        );
        if (extSub) {
          existingExternalSubtitleIndex = extSub.index;
        }
      }

      const seasonPart =
        item.seasonName ||
        (item.seasonIndex !== undefined && item.seasonIndex !== null
          ? `Season ${item.seasonIndex}`
          : '');
      const episodePart = item.episodeIndex !== undefined ? `E${item.episodeIndex} - ` : '';
      const displayName = item.seriesName
        ? `${item.seriesName} ${seasonPart} ${episodePart}${item.name}`.replace(/\s+/g, ' ').trim()
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
    clearTimeout(noticeTimeoutRef.current ?? undefined);
    noticeTimeoutRef.current = window.setTimeout(() => {
      setSubmittedNotice(null);
      noticeTimeoutRef.current = null;
    }, 4000);

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
    selectAllFiltered,
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
