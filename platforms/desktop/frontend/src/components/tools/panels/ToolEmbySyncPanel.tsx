import { AlertTriangle, RefreshCw, Settings, Tv } from 'lucide-react';
import type React from 'react';
import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsTabContainer } from '../../settings/SettingsLayout';
import { ToolHeroHeader, type ToolHeroStatusTone } from '../ToolHeroHeader';
import { EmbyLibraryToolbar } from './emby/EmbyLibraryToolbar';
import { EmbyMediaRow } from './emby/EmbyMediaRow';
import { EmbyPluginGuideModal } from './emby/EmbyPluginGuideModal';
import { EmbySeasonGroup } from './emby/EmbySeasonGroup';
import { EmbyServerConfigModal } from './emby/EmbyServerConfigModal';
import { EmbySubmitBar } from './emby/EmbySubmitBar';
import { useEmbySync } from './emby/useEmbySync';
import './ToolEmbySyncPanel.css';

export function ToolEmbySyncPanel(): React.JSX.Element {
  const { t } = useTranslation();
  const {
    config,
    setConfig,
    serverInfo,
    isConnecting,
    connectionError,
    isConnected,
    handleConnect,

    isConfigModalOpen,
    setIsConfigModalOpen,
    isPluginModalOpen,
    setIsPluginModalOpen,

    libraries,
    selectedLibraryId,
    setSelectedLibraryId,
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
  } = useEmbySync();
  const selectableFilteredItems = useMemo(() => {
    return filteredItems.filter((item) => !queuedItemIds.has(item.id));
  }, [filteredItems, queuedItemIds]);

  const totalSelectableCount = selectableFilteredItems.length;
  const selectedInFilteredCount = useMemo(() => {
    return selectableFilteredItems.filter((item) => selectedItemIds.has(item.id)).length;
  }, [selectableFilteredItems, selectedItemIds]);

  const isAllSelected =
    totalSelectableCount > 0 && selectedInFilteredCount === totalSelectableCount;
  const isIndeterminate =
    selectedInFilteredCount > 0 && selectedInFilteredCount < totalSelectableCount;

  const handleToggleSelectAll = useCallback(() => {
    if (isAllSelected) {
      clearSelection();
    } else {
      selectAllFiltered(filteredItems);
    }
  }, [isAllSelected, clearSelection, selectAllFiltered, filteredItems]);

  const statusTone: ToolHeroStatusTone = isConnecting
    ? 'active'
    : !isConnected
      ? 'inactive'
      : serverInfo && !serverInfo.hasSubtitleManagerPlugin
        ? 'warning'
        : 'active';

  const statusText = isConnecting
    ? t('emby.connecting', { defaultValue: 'Connecting...' })
    : !isConnected || !serverInfo
      ? t('emby.not_connected', { defaultValue: 'Not Connected' })
      : `${serverInfo.serverName} (v${serverInfo.version})`;
  return (
    <SettingsTabContainer id="tools-panel-emby_sync" ariaLabelledby="tools-tab-emby_sync">
      <div className="emby-sync-panel">
        <ToolHeroHeader
          id="emby-hero-header"
          icon={<Tv size={20} />}
          title={t('emby.title', { defaultValue: 'Emby Subtitle Assistant' })}
          description={t('emby.subtitle', {
            defaultValue:
              'Detect missing subtitles in Emby media libraries, transcribe audio offline, and auto-complete subtitles',
          })}
          statusText={statusText}
          statusTone={statusTone}
          hideSwitch={true}
          actions={
            <div className="emby-header-actions">
              {isConnected && (
                <button
                  type="button"
                  className="btn btn-sm btn-secondary"
                  onClick={fetchItems}
                  disabled={isLoadingItems}
                >
                  <RefreshCw size={14} className={isLoadingItems ? 'spin' : ''} />
                  <span>{t('common.refresh', { defaultValue: 'Refresh' })}</span>
                </button>
              )}
              <button
                type="button"
                className="btn btn-sm btn-secondary"
                onClick={() => setIsConfigModalOpen(true)}
              >
                <Settings size={14} style={{ marginRight: 6 }} />
                <span>
                  {isConnected
                    ? t('emby.server_settings', { defaultValue: 'Server Settings' })
                    : t('emby.connect_server', { defaultValue: 'Connect Server' })}
                </span>
              </button>
            </div>
          }
        />

        {/* Plugin Missing Alert Banner */}
        {serverInfo && !serverInfo.hasSubtitleManagerPlugin && (
          <div className="emby-plugin-warning-bar" role="alert">
            <div className="emby-warning-content">
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
              className="btn btn-xs btn-secondary"
              onClick={() => setIsPluginModalOpen(true)}
            >
              {t('emby.get_plugin', { defaultValue: 'Plugin Guide' })}
            </button>
          </div>
        )}

        {/* Main Content Area */}
        <div className="emby-sync-main-content">
          {isConnecting ? (
            <div
              className="emby-loading-state"
              style={{ margin: 'auto' }}
              role="status"
              aria-live="polite"
            >
              <RefreshCw size={28} className="spin" aria-hidden="true" />
              <p>{t('emby.connecting', { defaultValue: 'Connecting to Emby Server...' })}</p>
            </div>
          ) : !isConnected ? (
            <div className="emby-disconnected-card">
              <div className="emby-empty-icon-wrap">
                <Tv size={36} />
              </div>
              <h3>{t('emby.welcome_title', { defaultValue: 'Connect to your Emby Server' })}</h3>
              {connectionError && (
                <div
                  className="emby-alert-box emby-alert-error"
                  role="alert"
                  style={{ maxWidth: 460, margin: '0 auto', textAlign: 'left' }}
                >
                  <AlertTriangle size={15} style={{ flexShrink: 0 }} />
                  <span>{connectionError}</span>
                </div>
              )}
              <p>
                {t('emby.welcome_desc', {
                  defaultValue:
                    'Connect your Emby media server with an admin API key to browse libraries, select movies or TV series, transcribe audio offline with Sona, and sync subtitles directly back.',
                })}
              </p>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => setIsConfigModalOpen(true)}
              >
                <Settings size={15} style={{ marginRight: 6 }} />
                <span>{t('emby.configure_now', { defaultValue: 'Configure Server' })}</span>
              </button>
            </div>
          ) : (
            <>
              <EmbyLibraryToolbar
                libraries={libraries}
                selectedLibraryId={selectedLibraryId}
                onSelectLibrary={setSelectedLibraryId}
                searchTerm={searchTerm}
                onSearchChange={setSearchTerm}
                targetLanguage={targetLanguage}
                onTargetLanguageChange={setTargetLanguage}
                onlyMissingTargetSubtitles={onlyMissingTargetSubtitles}
                onOnlyMissingTargetSubtitlesChange={setOnlyMissingTargetSubtitles}
                isAllSelected={isAllSelected}
                isIndeterminate={isIndeterminate}
                selectedCount={selectedInFilteredCount}
                totalSelectableCount={totalSelectableCount}
                onToggleSelectAll={handleToggleSelectAll}
              />

              <div className="emby-media-list-container">
                {isLoadingItems ? (
                  <div className="emby-loading-state" role="status" aria-live="polite">
                    <RefreshCw size={24} className="spin" aria-hidden="true" />
                    <p>
                      {t('emby.loading_items', { defaultValue: 'Loading media library items...' })}
                    </p>
                  </div>
                ) : filteredItems.length === 0 ? (
                  <div className="emby-empty-media-card">
                    <p>
                      {t('emby.no_items_found', {
                        defaultValue: 'No media items found matching the current filter.',
                      })}
                    </p>
                  </div>
                ) : (
                  <div className="emby-media-groups-list">
                    {/* Standalone Movies */}
                    {groupedEpisodes.standalone.length > 0 && (
                      <div className="emby-standalone-list">
                        {groupedEpisodes.standalone.map((item) => (
                          <EmbyMediaRow
                            key={item.id}
                            item={item}
                            targetLanguage={targetLanguage}
                            isSelected={selectedItemIds.has(item.id)}
                            isQueued={queuedItemIds.has(item.id)}
                            selectedAudioIndex={audioTrackMap[item.id]}
                            onToggleSelect={toggleItemSelection}
                            onAudioTrackChange={handleAudioTrackChange}
                          />
                        ))}
                      </div>
                    )}

                    {/* TV Season Groups */}
                    {Object.entries(groupedEpisodes.groups).map(([seasonKey, seasonItems]) => (
                      <EmbySeasonGroup
                        key={seasonKey}
                        seasonKey={seasonKey}
                        seasonItems={seasonItems}
                        isExpanded={expandedSeasons.has(seasonKey)}
                        onToggleExpand={toggleSeasonExpansion}
                        targetLanguage={targetLanguage}
                        selectedItemIds={selectedItemIds}
                        queuedItemIds={queuedItemIds}
                        audioTrackMap={audioTrackMap}
                        onToggleSelect={toggleItemSelection}
                        onAudioTrackChange={handleAudioTrackChange}
                        onSelectAllMissingInSeason={selectAllMissingInSeason}
                      />
                    ))}
                  </div>
                )}
              </div>

              <EmbySubmitBar
                selectedCount={selectedInFilteredCount}
                subtitleMode={subtitleMode}
                onSubtitleModeChange={setSubtitleMode}
                overwriteExistingExternal={overwriteExistingExternal}
                onOverwriteExistingExternalChange={setOverwriteExistingExternal}
                onSubmitBatch={handleSubmitBatch}
                submittedNotice={submittedNotice}
              />
            </>
          )}
        </div>

        {/* Modals */}
        <EmbyServerConfigModal
          isOpen={isConfigModalOpen}
          onClose={() => setIsConfigModalOpen(false)}
          config={config}
          onChangeConfig={setConfig}
          isConnecting={isConnecting}
          connectionError={connectionError}
          onConnect={handleConnect}
        />

        <EmbyPluginGuideModal
          isOpen={isPluginModalOpen}
          onClose={() => setIsPluginModalOpen(false)}
        />
      </div>
    </SettingsTabContainer>
  );
}
