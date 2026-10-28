import {
  AlertCircle,
  AlertTriangle,
  Check,
  ChevronDown,
  ChevronRight,
  Download,
  ExternalLink,
  Film,
  RefreshCw,
  Search,
  Settings,
  Tv,
  X,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getEmbyConfig,
  getEmbyItems,
  getEmbyLibraries,
  saveEmbyConfig,
  testEmbyConnection,
} from '../../../services/emby/embyService';
import { useBatchQueueStore } from '../../../stores/batchQueueStore';
import type { EmbySubtitleMode } from '../../../types/batchQueue';
import type {
  EmbyConnectionConfig,
  EmbyLibrary,
  EmbyMediaItem,
  EmbyServerInfo,
  EmbyStreamInfo,
} from '../../../types/emby';
import { extractErrorMessage } from '../../../utils/errorUtils';
import './ToolEmbySyncPanel.css';

function formatDurationTicks(ticks?: number): string {
  if (!ticks) return '';
  const seconds = Math.floor(ticks / 10_000_000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

function getAudioTrackLabel(stream: EmbyStreamInfo): string {
  const parts: string[] = [];
  if (stream.displayTitle) {
    parts.push(stream.displayTitle);
  } else {
    if (stream.language) parts.push(stream.language.toUpperCase());
    if (stream.codec) parts.push(stream.codec.toUpperCase());
  }
  if (stream.isDefault) parts.push('(Default)');
  return parts.join(' ') || `Track #${stream.index}`;
}

function findBestAudioIndex(streams: EmbyStreamInfo[], preferredLang: string): number {
  const audios = streams.filter((s) => s.streamType.toLowerCase() === 'audio');
  if (audios.length === 0) return 0;

  if (preferredLang && preferredLang !== 'auto') {
    const langLower = preferredLang.toLowerCase();
    const matched = audios.find((s) => {
      const l = (s.language || '').toLowerCase();
      return (
        l === langLower ||
        (langLower === 'ja' && (l === 'jpn' || l === 'japanese')) ||
        (langLower === 'en' && (l === 'eng' || l === 'english')) ||
        (langLower === 'zh' && (l === 'chi' || l === 'zho' || l.startsWith('zh'))) ||
        (langLower === 'ko' && (l === 'kor' || l === 'korean'))
      );
    });
    if (matched) return matched.index;
  }

  const def = audios.find((s) => s.isDefault);
  return def ? def.index : audios[0].index;
}

export function ToolEmbySyncPanel(): React.JSX.Element {
  const { t } = useTranslation();

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
  const [onlyMissingChinese, setOnlyMissingChinese] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [items, setItems] = useState<EmbyMediaItem[]>([]);
  const [isLoadingItems, setIsLoadingItems] = useState(false);

  // Selection & Track Map
  const [selectedItemIds, setSelectedItemIds] = useState<Set<string>>(new Set());
  const [audioTrackMap, setAudioTrackMap] = useState<Record<string, number>>({});
  const [expandedSeasons, setExpandedSeasons] = useState<Set<string>>(new Set());

  // Global Strategy Options
  const [sourceLanguage, setSourceLanguage] = useState<string>('auto');
  const [targetLanguage, setTargetLanguage] = useState<string>('zh-CN');
  const [subtitleMode, setSubtitleMode] = useState<EmbySubtitleMode>('bilingual');
  const [overwriteExistingExternal, setOverwriteExistingExternal] = useState(false);
  const [submittedNotice, setSubmittedNotice] = useState<string | null>(null);

  // 1. Connect to Emby Server
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

        // Load media libraries
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

  // 2. Load initial saved config
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

  // 3. Load items when library or search changes
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

      // Initialize audio tracks based on current preferred sourceLanguage
      const newMap: Record<string, number> = {};
      const seasons = new Set<string>();
      for (const item of res.items) {
        newMap[item.id] = findBestAudioIndex(item.mediaStreams, sourceLanguage);
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
  }, [serverInfo, selectedLibraryId, searchTerm, sourceLanguage]);

  useEffect(() => {
    if (serverInfo) {
      fetchItems();
    }
  }, [serverInfo, fetchItems]);

  // Filter items
  const filteredItems = useMemo(() => {
    if (!onlyMissingChinese) return items;
    return items.filter((item) => !item.hasChineseSubtitles);
  }, [items, onlyMissingChinese]);

  // Group items by Season for TV Shows, or keep flat for Movies
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
    return { groups, standalone };
  }, [filteredItems]);

  // Selection helpers
  const toggleItemSelection = (id: string) => {
    setSelectedItemIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAllMissingInSeason = (seasonItems: EmbyMediaItem[]) => {
    setSelectedItemIds((prev) => {
      const next = new Set(prev);
      for (const it of seasonItems) {
        if (!it.hasChineseSubtitles) {
          next.add(it.id);
        }
      }
      return next;
    });
  };

  const toggleSeasonExpansion = (key: string) => {
    setExpandedSeasons((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const handleAudioTrackChange = (itemId: string, streamIndex: number) => {
    setAudioTrackMap((prev) => ({ ...prev, [itemId]: streamIndex }));
  };

  // Submit to Batch Queue
  const handleSubmitBatch = () => {
    if (selectedItemIds.size === 0 || !serverInfo) return;

    const entries = [];
    for (const id of selectedItemIds) {
      const item = items.find((it) => it.id === id);
      if (!item) continue;

      const audioStreamIndex =
        audioTrackMap[item.id] ?? findBestAudioIndex(item.mediaStreams, sourceLanguage);

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
        sourceLanguage: sourceLanguage === 'auto' ? undefined : sourceLanguage,
        existingExternalSubtitleIndex,
        serverId: serverInfo.id,
        serverUrl: config.serverUrl,
      });
    }

    useBatchQueueStore.getState().addEmbyItems({ items: entries });
    setSelectedItemIds(new Set());
    setSubmittedNotice(
      t('emby.submitted_success', {
        count: entries.length,
        defaultValue: `Successfully added ${entries.length} tasks to the transcription queue`,
      })
    );
    setTimeout(() => setSubmittedNotice(null), 4000);
  };

  const isConnected = Boolean(serverInfo);

  return (
    <div className="emby-sync-panel">
      {/* 1. Top Header & Server Connection */}
      <div className="emby-sync-header">
        <div className="emby-sync-identity">
          <div className="emby-sync-icon-tile">
            <Tv size={22} />
          </div>
          <div className="emby-sync-title-group">
            <h2>
              {t('emby.title', { defaultValue: 'Emby Subtitle Sync' })}
              {serverInfo && (
                <span className="emby-status-pill connected">
                  <span className="emby-status-dot" />
                  <span>
                    {serverInfo.serverName} (v{serverInfo.version})
                  </span>
                </span>
              )}
              {!serverInfo && (
                <span className="emby-status-pill disconnected">
                  <span className="emby-status-dot" />
                  <span>{t('emby.not_connected', { defaultValue: 'Not Connected' })}</span>
                </span>
              )}
            </h2>
            <p>
              {t('emby.subtitle', {
                defaultValue:
                  'Pull audio streams, transcribe offline, and sync subtitles back to Emby media libraries',
              })}
            </p>
          </div>
        </div>

        <div className="emby-sync-header-controls">
          {isConnected && (
            <button
              type="button"
              className="btn btn-sm btn-secondary"
              onClick={fetchItems}
              disabled={isLoadingItems}
              title={t('common.refresh', { defaultValue: 'Refresh' })}
            >
              <RefreshCw size={14} className={isLoadingItems ? 'spin' : ''} />
            </button>
          )}
          <button
            type="button"
            className="btn btn-sm btn-secondary"
            onClick={() => setIsConfigModalOpen(true)}
          >
            <Settings size={14} style={{ marginRight: 6 }} />
            {isConnected
              ? t('emby.server_settings', { defaultValue: 'Server Settings' })
              : t('emby.connect_server', { defaultValue: 'Connect Server' })}
          </button>
        </div>
      </div>

      {/* Plugin Missing Alert Banner */}
      {serverInfo && !serverInfo.hasSubtitleManagerPlugin && (
        <div className="emby-plugin-warning-bar">
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <AlertTriangle size={16} />
            <span>
              {t('emby.plugin_missing_warning', {
                defaultValue:
                  'Server plugin Emby.SubtitleManager is not detected. Automatic subtitle upload will be limited.',
              })}
            </span>
          </div>
          <button
            type="button"
            className="btn btn-sm btn-warning"
            onClick={() => setIsPluginModalOpen(true)}
          >
            <Download size={14} style={{ marginRight: 4 }} />
            {t('emby.get_plugin', { defaultValue: 'Plugin Guide' })}
          </button>
        </div>
      )}

      {submittedNotice && (
        <div
          style={{
            background: 'rgba(82, 196, 26, 0.15)',
            borderBottom: '1px solid rgba(82, 196, 26, 0.3)',
            padding: '8px 24px',
            color: '#52c41a',
            fontSize: '13px',
            display: 'flex',
            alignItems: 'center',
            gap: 8,
          }}
        >
          <Check size={16} />
          <span>{submittedNotice}</span>
        </div>
      )}

      {/* 2. Main Workspace Layout */}
      <div className="emby-sync-body">
        {/* Left Sub-Sidebar */}
        <div className="emby-sync-sidebar">
          <div className="emby-filter-box">
            <div style={{ position: 'relative' }}>
              <input
                type="text"
                className="emby-search-input"
                placeholder={t('emby.search_placeholder', {
                  defaultValue: 'Search movies and TV shows...',
                })}
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
              />
              <Search
                size={14}
                style={{
                  position: 'absolute',
                  right: 8,
                  top: 9,
                  color: 'var(--color-text-tertiary)',
                  pointerEvents: 'none',
                }}
              />
            </div>
            <label className="emby-filter-checkbox">
              <input
                type="checkbox"
                checked={onlyMissingChinese}
                onChange={(e) => setOnlyMissingChinese(e.target.checked)}
              />
              <span>
                {t('emby.only_missing_chinese', { defaultValue: 'Only missing Chinese subtitles' })}
              </span>
            </label>
          </div>

          <div
            style={{
              fontSize: 11,
              fontWeight: 600,
              color: 'var(--color-text-tertiary)',
              textTransform: 'uppercase',
            }}
          >
            {t('emby.libraries', { defaultValue: 'Libraries' })}
          </div>
          <div className="emby-library-list">
            {libraries.map((lib) => (
              <button
                key={lib.id}
                type="button"
                className={`emby-library-item ${selectedLibraryId === lib.id ? 'active' : ''}`}
                onClick={() => setSelectedLibraryId(lib.id)}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  {lib.collectionType === 'tvshows' ? <Tv size={14} /> : <Film size={14} />}
                  <span>{lib.name}</span>
                </div>
              </button>
            ))}
            {libraries.length === 0 && (
              <div style={{ color: 'var(--color-text-tertiary)', fontSize: 12, padding: 8 }}>
                {isConnected
                  ? t('emby.no_libraries', { defaultValue: 'No libraries found' })
                  : t('emby.connect_prompt', { defaultValue: 'Please connect to server first' })}
              </div>
            )}
          </div>
        </div>

        {/* Right Main List */}
        <div className="emby-sync-main">
          {!isConnected ? (
            <div
              style={{
                flex: 1,
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 16,
                color: 'var(--color-text-tertiary)',
              }}
            >
              <Tv size={48} style={{ opacity: 0.5 }} />
              <div style={{ textAlign: 'center' }}>
                <h4 style={{ margin: '0 0 8px', color: 'var(--color-text-primary)' }}>
                  {t('emby.welcome_title', { defaultValue: 'Connect to your Emby media server' })}
                </h4>
                <p style={{ margin: 0, fontSize: 13, maxWidth: 360 }}>
                  {t('emby.welcome_desc', {
                    defaultValue:
                      'Enter your server URL and API key to browse media, pick audio tracks, and sync subtitles in one click.',
                  })}
                </p>
              </div>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => setIsConfigModalOpen(true)}
              >
                {t('emby.configure_now', { defaultValue: 'Configure Connection' })}
              </button>
            </div>
          ) : isLoadingItems ? (
            <div
              style={{
                flex: 1,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 8,
                color: 'var(--color-text-secondary)',
              }}
            >
              <RefreshCw size={18} className="spin" />
              <span>{t('common.loading', { defaultValue: 'Loading media items...' })}</span>
            </div>
          ) : filteredItems.length === 0 ? (
            <div
              style={{
                flex: 1,
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 8,
                color: 'var(--color-text-tertiary)',
              }}
            >
              <Film size={36} style={{ opacity: 0.4 }} />
              <span>
                {t('emby.no_items_found', { defaultValue: 'No matching media items found' })}
              </span>
            </div>
          ) : (
            <>
              {/* Standalone Movies / Flat items */}
              {groupedEpisodes.standalone.length > 0 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                  <div className="emby-items-header">
                    <h3>{t('emby.movies_section', { defaultValue: 'Movies' })}</h3>
                  </div>
                  {groupedEpisodes.standalone.map((item) => {
                    const isSelected = selectedItemIds.has(item.id);
                    const audios = item.mediaStreams.filter(
                      (s) => s.streamType.toLowerCase() === 'audio'
                    );
                    const currentAudioIndex =
                      audioTrackMap[item.id] ??
                      findBestAudioIndex(item.mediaStreams, sourceLanguage);

                    return (
                      <div
                        key={item.id}
                        className={`emby-item-card ${isSelected ? 'selected' : ''}`}
                      >
                        <div className="emby-item-left">
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => toggleItemSelection(item.id)}
                            style={{ cursor: 'pointer' }}
                          />
                          <div className="emby-item-meta">
                            <div className="emby-item-title" title={item.name}>
                              {item.name}
                            </div>
                            <div className="emby-item-badges">
                              {item.hasChineseSubtitles ? (
                                <span className="emby-badge has-zh">
                                  {item.hasEmbeddedChinese
                                    ? t('emby.badge_embedded_zh', { defaultValue: 'Embedded ZH' })
                                    : t('emby.badge_external_zh', { defaultValue: 'External ZH' })}
                                </span>
                              ) : (
                                <span className="emby-badge missing-zh">
                                  {t('emby.badge_missing_zh', {
                                    defaultValue: 'No Chinese Subtitle',
                                  })}
                                </span>
                              )}
                              {item.runtimeTicks && (
                                <span className="emby-badge info">
                                  {formatDurationTicks(item.runtimeTicks)}
                                </span>
                              )}
                            </div>
                          </div>
                        </div>

                        <div className="emby-item-right">
                          {audios.length > 0 && (
                            <select
                              className="emby-audio-select"
                              value={currentAudioIndex}
                              onChange={(e) =>
                                handleAudioTrackChange(item.id, Number(e.target.value))
                              }
                            >
                              {audios.map((a) => (
                                <option key={a.index} value={a.index}>
                                  {getAudioTrackLabel(a)}
                                </option>
                              ))}
                            </select>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}

              {/* Series / Season Accordion */}
              {Object.entries(groupedEpisodes.groups).map(([seasonKey, seasonItems]) => {
                const isExpanded = expandedSeasons.has(seasonKey);
                const missingCount = seasonItems.filter((i) => !i.hasChineseSubtitles).length;

                return (
                  <div key={seasonKey} className="emby-season-group">
                    <div
                      className="emby-season-header"
                      onClick={() => toggleSeasonExpansion(seasonKey)}
                    >
                      <div className="emby-season-title">
                        {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                        <span>{seasonKey}</span>
                        <span style={{ fontSize: 12, color: 'var(--color-text-tertiary)' }}>
                          {t('emby.episodes_count', {
                            count: seasonItems.length,
                            defaultValue: `(${seasonItems.length} episodes)`,
                          })}
                        </span>
                        {missingCount > 0 ? (
                          <span className="emby-badge missing-zh">
                            {t('emby.missing_zh_count', {
                              count: missingCount,
                              defaultValue: `${missingCount} missing Chinese`,
                            })}
                          </span>
                        ) : (
                          <span className="emby-badge has-zh">
                            {t('emby.all_have_zh', { defaultValue: 'All have Chinese' })}
                          </span>
                        )}
                      </div>

                      <div onClick={(e) => e.stopPropagation()}>
                        <button
                          type="button"
                          className="btn btn-xs btn-secondary"
                          onClick={() => selectAllMissingInSeason(seasonItems)}
                        >
                          {t('emby.select_season_missing', {
                            defaultValue: 'Select Missing Episodes in Season',
                          })}
                        </button>
                      </div>
                    </div>

                    {isExpanded && (
                      <div className="emby-season-episodes">
                        {seasonItems.map((item) => {
                          const isSelected = selectedItemIds.has(item.id);
                          const audios = item.mediaStreams.filter(
                            (s) => s.streamType.toLowerCase() === 'audio'
                          );
                          const currentAudioIndex =
                            audioTrackMap[item.id] ??
                            findBestAudioIndex(item.mediaStreams, sourceLanguage);

                          return (
                            <div
                              key={item.id}
                              className={`emby-item-card ${isSelected ? 'selected' : ''}`}
                            >
                              <div className="emby-item-left">
                                <input
                                  type="checkbox"
                                  checked={isSelected}
                                  onChange={() => toggleItemSelection(item.id)}
                                  style={{ cursor: 'pointer' }}
                                />
                                <div className="emby-item-meta">
                                  <div className="emby-item-title">
                                    {item.episodeIndex != null ? `E${item.episodeIndex} - ` : ''}
                                    {item.name}
                                  </div>
                                  <div className="emby-item-badges">
                                    {item.hasChineseSubtitles ? (
                                      <span className="emby-badge has-zh">
                                        {item.hasEmbeddedChinese
                                          ? t('emby.badge_embedded_zh', {
                                              defaultValue: 'Embedded ZH',
                                            })
                                          : t('emby.badge_external_zh', {
                                              defaultValue: 'External ZH',
                                            })}
                                      </span>
                                    ) : (
                                      <span className="emby-badge missing-zh">
                                        {t('emby.badge_missing_zh', {
                                          defaultValue: 'No Chinese Subtitle',
                                        })}
                                      </span>
                                    )}
                                    {item.runtimeTicks && (
                                      <span className="emby-badge info">
                                        {formatDurationTicks(item.runtimeTicks)}
                                      </span>
                                    )}
                                  </div>
                                </div>
                              </div>

                              <div className="emby-item-right">
                                {audios.length > 0 && (
                                  <select
                                    className="emby-audio-select"
                                    value={currentAudioIndex}
                                    onChange={(e) =>
                                      handleAudioTrackChange(item.id, Number(e.target.value))
                                    }
                                  >
                                    {audios.map((a) => (
                                      <option key={a.index} value={a.index}>
                                        {getAudioTrackLabel(a)}
                                      </option>
                                    ))}
                                  </select>
                                )}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })}
            </>
          )}
        </div>
      </div>

      {/* 3. Bottom Global Strategy Bar */}
      {isConnected && (
        <div className="emby-sync-footer">
          <div className="emby-strategy-controls">
            <div className="emby-strategy-item">
              <span>{t('emby.source_lang', { defaultValue: 'Source Language:' })}</span>
              <select value={sourceLanguage} onChange={(e) => setSourceLanguage(e.target.value)}>
                <option value="auto">{t('emby.lang_auto', { defaultValue: 'Auto Detect' })}</option>
                <option value="en">English</option>
                <option value="ja">日本語 (Japanese)</option>
                <option value="ko">한국어 (Korean)</option>
                <option value="zh">中文 (Chinese)</option>
              </select>
            </div>

            <div className="emby-strategy-item">
              <span>{t('emby.target_lang', { defaultValue: 'Target Language:' })}</span>
              <select value={targetLanguage} onChange={(e) => setTargetLanguage(e.target.value)}>
                <option value="zh-CN">简体中文 (zh-CN)</option>
                <option value="zh-TW">繁體中文 (zh-TW)</option>
              </select>
            </div>

            <div className="emby-strategy-item">
              <span>{t('emby.subtitle_mode', { defaultValue: 'Subtitle Mode:' })}</span>
              <select
                value={subtitleMode}
                onChange={(e) => setSubtitleMode(e.target.value as EmbySubtitleMode)}
              >
                <option value="bilingual">
                  {t('emby.mode_bilingual', { defaultValue: 'Bilingual Subtitles' })}
                </option>
                <option value="pure_translation">
                  {t('emby.mode_pure', { defaultValue: 'Translation Only' })}
                </option>
                <option value="separate_tracks">
                  {t('emby.mode_separate', {
                    defaultValue: 'Separate Original & Translation Tracks',
                  })}
                </option>
              </select>
            </div>

            <label className="emby-filter-checkbox" style={{ marginLeft: 8 }}>
              <input
                type="checkbox"
                checked={overwriteExistingExternal}
                onChange={(e) => setOverwriteExistingExternal(e.target.checked)}
              />
              <span>
                {t('emby.overwrite_external', {
                  defaultValue: 'Overwrite existing external subtitle of same language',
                })}
              </span>
            </label>
          </div>

          <div>
            <button
              type="button"
              className="btn btn-primary"
              disabled={selectedItemIds.size === 0}
              onClick={handleSubmitBatch}
            >
              {t('emby.submit_btn', {
                count: selectedItemIds.size,
                defaultValue: `Transcribe and Sync (${selectedItemIds.size} selected)`,
              })}
            </button>
          </div>
        </div>
      )}

      {/* 4. Server Configuration Modal */}
      {isConfigModalOpen && (
        <div className="emby-modal-overlay" onClick={() => setIsConfigModalOpen(false)}>
          <div className="emby-modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="emby-modal-header">
              <h3>
                {t('emby.modal_config_title', { defaultValue: 'Configure Emby Server Connection' })}
              </h3>
              <button
                type="button"
                className="btn btn-icon btn-sm"
                onClick={() => setIsConfigModalOpen(false)}
              >
                <X size={16} />
              </button>
            </div>

            <div className="emby-modal-body">
              {connectionError && (
                <div
                  style={{
                    background: 'rgba(255, 77, 79, 0.12)',
                    color: '#ff4d4f',
                    padding: '8px 12px',
                    borderRadius: 'var(--radius-sm)',
                    fontSize: 12,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                  }}
                >
                  <AlertCircle size={14} />
                  <span>{connectionError}</span>
                </div>
              )}

              <label>
                <span>
                  {t('emby.modal_url_label', {
                    defaultValue: 'Server URL (including port and path)',
                  })}
                </span>
                <input
                  type="text"
                  placeholder="http://192.168.1.100:8096"
                  value={config.serverUrl}
                  onChange={(e) => setConfig({ ...config, serverUrl: e.target.value })}
                />
              </label>

              <label>
                <span>{t('emby.modal_key_label', { defaultValue: 'API Key (Admin token)' })}</span>
                <input
                  type="password"
                  placeholder="Emby API Key"
                  value={config.apiKey}
                  onChange={(e) => setConfig({ ...config, apiKey: e.target.value })}
                />
              </label>

              <label style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginTop: 4 }}>
                <input
                  type="checkbox"
                  checked={config.allowInvalidCerts}
                  onChange={(e) => setConfig({ ...config, allowInvalidCerts: e.target.checked })}
                />
                <span>
                  {t('emby.modal_cert_label', {
                    defaultValue: 'Allow self-signed or invalid SSL certificates (LAN HTTPS)',
                  })}
                </span>
              </label>
            </div>

            <div className="emby-modal-footer">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setIsConfigModalOpen(false)}
              >
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={isConnecting || !config.serverUrl || !config.apiKey}
                onClick={() => handleConnect()}
              >
                {isConnecting ? (
                  <>
                    <RefreshCw size={14} className="spin" style={{ marginRight: 6 }} />
                    {t('common.connecting', { defaultValue: 'Connecting...' })}
                  </>
                ) : (
                  t('emby.connect_and_save', { defaultValue: 'Test & Save' })
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 5. SubtitleManager Plugin Install Guide Modal */}
      {isPluginModalOpen && (
        <div className="emby-modal-overlay" onClick={() => setIsPluginModalOpen(false)}>
          <div className="emby-modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="emby-modal-header">
              <h3>
                {t('emby.plugin_modal_title', {
                  defaultValue: 'Install Emby.SubtitleManager Plugin',
                })}
              </h3>
              <button
                type="button"
                className="btn btn-icon btn-sm"
                onClick={() => setIsPluginModalOpen(false)}
              >
                <X size={16} />
              </button>
            </div>

            <div className="emby-modal-body">
              <p style={{ margin: 0, lineHeight: 1.5 }}>
                {t('emby.plugin_intro', {
                  defaultValue:
                    'Sona uses the open-source Emby.SubtitleManager plugin for seamless bidirectional subtitle uploads and metadata refresh. Follow these steps to deploy:',
                })}
              </p>

              <div
                style={{
                  background: 'var(--color-bg-secondary)',
                  padding: '12px',
                  borderRadius: 'var(--radius-sm)',
                }}
              >
                <div style={{ fontWeight: 600, marginBottom: 4 }}>
                  {t('emby.plugin_step1_title', {
                    defaultValue: 'Step 1: Download the plugin DLL',
                  })}
                </div>
                <div
                  style={{ fontSize: 12, color: 'var(--color-text-secondary)', marginBottom: 8 }}
                >
                  {t('emby.plugin_step1_desc', {
                    defaultValue:
                      'Get the latest Emby.SubtitleManager.dll from the official GitHub Release:',
                  })}
                </div>
                <a
                  href="https://github.com/dianbaer/Emby.SubtitleManager/releases"
                  target="_blank"
                  rel="noreferrer"
                  className="btn btn-sm btn-secondary"
                  style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                >
                  <ExternalLink size={14} />
                  <span>
                    {t('emby.plugin_step1_btn', { defaultValue: 'Visit GitHub Releases' })}
                  </span>
                </a>
              </div>

              <div
                style={{
                  background: 'var(--color-bg-secondary)',
                  padding: '12px',
                  borderRadius: 'var(--radius-sm)',
                }}
              >
                <div style={{ fontWeight: 600, marginBottom: 4 }}>
                  {t('emby.plugin_step2_title', {
                    defaultValue: 'Step 2: Place the DLL in Emby plugins folder and restart',
                  })}
                </div>
                <ul
                  style={{
                    margin: '4px 0 0 16px',
                    padding: 0,
                    fontSize: 12,
                    color: 'var(--color-text-secondary)',
                    lineHeight: 1.6,
                  }}
                >
                  <li>
                    {t('emby.plugin_docker', {
                      defaultValue: 'Docker deployment: mount volume path /config/plugins/',
                    })}
                  </li>
                  <li>
                    {t('emby.plugin_synology', {
                      defaultValue: 'Synology NAS: /var/packages/EmbyServer/var/plugins/',
                    })}
                  </li>
                  <li>
                    {t('emby.plugin_windows', {
                      defaultValue:
                        'Windows Server: %APPDATA%\\Emby-Server\\programdata\\plugins\\',
                    })}
                  </li>
                  <li>
                    {t('emby.plugin_linux', {
                      defaultValue: 'Linux Native: /var/lib/emby/plugins/',
                    })}
                  </li>
                </ul>
              </div>

              <p style={{ margin: 0, fontSize: 12, color: 'var(--color-text-tertiary)' }}>
                {t('emby.plugin_tip', {
                  defaultValue:
                    'Tip: After copying the DLL and restarting Emby, return to Sona and reconnect to activate full sync capabilities.',
                })}
              </p>
            </div>

            <div className="emby-modal-footer">
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => setIsPluginModalOpen(false)}
              >
                {t('common.close', { defaultValue: 'Close' })}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
