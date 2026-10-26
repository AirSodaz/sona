import { Check, Loader2, Plus, RefreshCw } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type {
  LocalLlmCardsResponse,
  LocalLlmModelCard as LocalLlmModelCardType,
} from '../../../bindings';
import { formatBytes } from '../../../hooks/notifications/modelDownloadAdapter';
import { parseDownloadProgressPayload } from '../../../services/modelDownloadService';
import {
  cleanInterruptedDownload,
  deletePresetModel,
  downloadPresetModel,
} from '../../../services/tauri/app';
import { TauriEvent } from '../../../services/tauri/events';
import {
  completeLlm,
  importLocalLlmFile,
  listLocalLlmCards,
  listLocalNmtCards,
  testLocalNmtModel,
} from '../../../services/tauri/llm';
import { openDialog } from '../../../services/tauri/platform/dialog';
import { listen } from '../../../services/tauri/platform/events';
import { useConfigStore } from '../../../stores/configStore';
import { useDialogStore } from '../../../stores/dialogStore';
import { useModelDownloadStore } from '../../../stores/modelDownloadStore';
import type { LlmAssistantConfig } from '../../../types/config';
import { normalizeError } from '../../../utils/errorUtils';
import { SegmentedSubTabs } from '../SecondaryTabs';
import { SettingsAccordion } from '../SettingsLayout';
import {
  type LocalDownloadProgressState,
  LocalModelCard,
  type LocalModelTestState,
} from './LocalModelCard';
import './LocalModelCard.css';

interface LocalProviderAccordionItemProps {
  config: LlmAssistantConfig;
  isOpen: boolean;
  onToggle: () => void;
  applyLlmSettings?: (nextSettings: LlmAssistantConfig['llmSettings']) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

export function LocalProviderAccordionItem({
  config: _config,
  isOpen,
  onToggle,
  applyLlmSettings: _applyLlmSettings,
  t,
}: LocalProviderAccordionItemProps): React.JSX.Element {
  const modelDownloadMirror = useConfigStore((state) => state.config.modelDownloadMirror);
  const [activeSubTab, setActiveSubTab] = useState<'llm' | 'nmt'>('llm');
  const [llmData, setLlmData] = useState<LocalLlmCardsResponse>({ modelsDir: '', cards: [] });
  const [nmtData, setNmtData] = useState<LocalLlmCardsResponse>({ modelsDir: '', cards: [] });
  const [isLoading, setIsLoading] = useState(false);
  const [downloads, setDownloads] = useState<Record<string, LocalDownloadProgressState>>({});
  const [modelTestStates, setModelTestStates] = useState<Record<string, LocalModelTestState>>({});
  const [inFlightActionIds, setInFlightActionIds] = useState<Set<string>>(new Set());
  const activeDownloadsRef = useRef(downloads);
  activeDownloadsRef.current = downloads;
  const inFlightDownloadsRef = useRef<Set<string>>(new Set());

  const confirm = useDialogStore((state) => state.confirm);
  const showError = useDialogStore((state) => state.showError);

  const fetchLlmCards = useCallback(async () => {
    try {
      const response = await listLocalLlmCards();
      if (response && Array.isArray(response.cards)) {
        setLlmData(response);
      }
    } catch (err) {
      console.error('Failed to list local LLM cards:', err);
    }
  }, []);

  const fetchNmtCards = useCallback(async () => {
    try {
      const response = await listLocalNmtCards();
      if (response && Array.isArray(response.cards)) {
        setNmtData(response);
      }
    } catch (err) {
      console.error('Failed to list local NMT cards:', err);
    }
  }, []);

  const fetchCards = useCallback(async () => {
    setIsLoading(true);
    try {
      await Promise.all([fetchLlmCards(), fetchNmtCards()]);
    } finally {
      setIsLoading(false);
    }
  }, [fetchLlmCards, fetchNmtCards]);
  useEffect(() => {
    fetchCards();
  }, [fetchCards]);

  // Listen to download-progress events
  useEffect(() => {
    const isTauri =
      typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
    if (!isTauri) return;

    let unlistenFn: (() => void) | undefined;
    let isCancelled = false;
    listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
      const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);
      if (!id || total <= 0) return;

      setDownloads((prev) => {
        if (!prev[id]) return prev;
        const progress = Math.min(100, Math.round((downloaded / total) * 100));
        return {
          ...prev,
          [id]: {
            ...prev[id],
            progress,
            downloadedBytes: downloaded,
            totalBytes: total,
          },
        };
      });
    })
      .then((unsub) => {
        if (isCancelled) {
          unsub();
        } else {
          unlistenFn = unsub;
        }
      })
      .catch(() => {});

    return () => {
      isCancelled = true;
      unlistenFn?.();
    };
  }, []);

  const handleCancelDownload = useCallback(
    async (cardId: string) => {
      try {
        await useModelDownloadStore.getState().cancelDownload(cardId);
      } catch (err) {
        console.error(`Failed to cancel download for ${cardId}:`, err);
      } finally {
        setDownloads((prev) => {
          const next = { ...prev };
          delete next[cardId];
          return next;
        });
        await fetchCards();
      }
    },
    [fetchCards]
  );

  const handleDownload = useCallback(
    async (card: LocalLlmModelCardType) => {
      const downloadId = card.id;
      if (
        inFlightDownloadsRef.current.has(downloadId) ||
        activeDownloadsRef.current[downloadId] ||
        useModelDownloadStore.getState().downloads[card.id]?.status === 'downloading'
      ) {
        return;
      }
      inFlightDownloadsRef.current.add(downloadId);

      useModelDownloadStore.getState().startDownload({
        modelId: card.id,
        modelName: card.name,
        modelType: 'llm',
        downloadId,
        onRetry: () => {
          const freshCard = cardsRef.current?.find((c) => c.id === card.id) ?? card;
          void handleDownload(freshCard);
        },
        onCancel: () => {
          setDownloads((prev) => {
            if (!prev[downloadId]) return prev;
            const next = { ...prev };
            delete next[downloadId];
            return next;
          });
        },
      });

      setDownloads((prev) => ({
        ...prev,
        [downloadId]: { progress: 0, statusText: t('settings.llm.status_downloading') },
      }));

      try {
        await downloadPresetModel({
          modelId: card.id,
          downloadId,
          mirror: modelDownloadMirror,
        });
        useModelDownloadStore.getState().completeDownload(card.id, downloadId);
        try {
          await fetchCards();
        } catch (cardErr) {
          console.warn('Failed to refresh cards after download:', cardErr);
        }
      } catch (err) {
        const errorMsg = normalizeError(err).message;
        if (errorMsg === 'Download cancelled') {
          void useModelDownloadStore.getState().cancelDownload(card.id, downloadId);
        } else {
          console.error(`Failed to download model ${card.id}:`, err);
          useModelDownloadStore.getState().failDownload(card.id, errorMsg, downloadId);
        }
        await fetchCards();
        return;
      } finally {
        inFlightDownloadsRef.current.delete(downloadId);
        setDownloads((prev) => {
          const next = { ...prev };
          delete next[downloadId];
          return next;
        });
      }
    },
    [fetchCards, modelDownloadMirror, t]
  );

  const handleDelete = useCallback(
    async (card: LocalLlmModelCardType) => {
      const confirmed = await confirm(
        t('settings.llm.delete_model_confirm', {
          name: card.name,
          defaultValue: `Are you sure you want to delete ${card.name}?`,
        })
      );
      if (!confirmed) return;

      try {
        await deletePresetModel(card.id);
        await fetchCards();
        setModelTestStates((prev) => {
          const next = { ...prev };
          delete next[card.id];
          return next;
        });
      } catch (err) {
        console.error(`Failed to delete model ${card.id}:`, err);
      }
    },
    [confirm, fetchCards, t]
  );

  const handleDiscardDownload = useCallback(
    async (card: LocalLlmModelCardType) => {
      if (
        activeDownloadsRef.current[card.id] ||
        inFlightDownloadsRef.current.has(card.id) ||
        useModelDownloadStore.getState().downloads[card.id]?.status === 'downloading' ||
        !card.interruptedDownload
      ) {
        return;
      }

      inFlightDownloadsRef.current.add(card.id);
      setInFlightActionIds((prev) => new Set(prev).add(card.id));
      try {
        const { downloadedBytes, totalBytes, percentage } = card.interruptedDownload;
        const downloadedFormatted = formatBytes(downloadedBytes) || '0 MB';
        const totalFormatted = totalBytes ? formatBytes(totalBytes) : '';
        const sizeText = totalFormatted
          ? `${downloadedFormatted} / ${totalFormatted}`
          : downloadedFormatted;
        const pctText = percentage != null ? `${Math.round(percentage)}%` : '';
        const progressDetail = pctText ? `${sizeText} (${pctText})` : sizeText;

        const confirmed = await confirm(
          t('settings.discard_download_confirm_message', {
            name: card.name,
            progress: progressDetail,
            defaultValue: `Are you sure you want to discard the incomplete download for ${card.name} (${progressDetail})? This will delete partial files and free up disk space.`,
          }),
          {
            title: t('settings.discard_download_confirm_title', {
              defaultValue: 'Discard Incomplete Download',
            }),
            confirmLabel: t('settings.discard_download_confirm_button', {
              defaultValue: 'Discard Files',
            }),
            cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
            variant: 'warning',
          }
        );

        if (!confirmed) return;

        if (
          activeDownloadsRef.current[card.id] ||
          useModelDownloadStore.getState().downloads[card.id]?.status === 'downloading'
        ) {
          return;
        }

        try {
          await cleanInterruptedDownload(card.id);
          useModelDownloadStore.getState().dismissDownload(card.id);
          await fetchCards();
        } catch (cleanErr) {
          console.warn('Failed to clean interrupted download:', cleanErr);
          await showError({
            code: 'model.clean_failed',
            messageKey: 'errors.model.clean_failed',
            cause: cleanErr,
          });
        }
      } finally {
        inFlightDownloadsRef.current.delete(card.id);
        setInFlightActionIds((prev) => {
          const next = new Set(prev);
          next.delete(card.id);
          return next;
        });
      }
    },
    [confirm, fetchCards, showError, t]
  );

  const handleImportCustomFile = useCallback(async () => {
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: 'GGUF Models', extensions: ['gguf'] }],
      });
      if (typeof selected === 'string' && selected) {
        await importLocalLlmFile(selected);
        await fetchCards();
      }
    } catch (err) {
      console.error('Failed to import file:', err);
    }
  }, [fetchCards]);

  const currentData = activeSubTab === 'llm' ? llmData : nmtData;
  const cards = useMemo(() => {
    if (currentData?.cards && currentData.cards.length > 0) {
      return currentData.cards;
    }
    if (activeSubTab === 'nmt') {
      return [];
    }
    return [
      {
        id: 'qwen3.5-4b',
        name: 'Qwen3.5 4B',
        model: 'Qwen/Qwen3.5-4B',
        filename: 'Qwen3.5-4B-Q4_K_M.gguf',
        description: 'settings.descriptions.qwen3_5_4b',
        backend: 'llama.cpp',
        contextWindow: 262144,
        maxOutputTokens: 4096,
        size: '~2.7 GB',
        parameters: '4B',
        quantization: 'Q4_K_M',
        modalities: ['text', 'image', 'video'],
        languages: ['zh', 'en', 'ja', 'ko'],
        capabilities: ['chat', 'reasoning', 'polish', 'summary', 'translate'],
        isRecommended: true,
        isInstalled: false,
        installedPath: null,
        installedSizeBytes: null,
        downloadUrl: null,
        downloadSizeBytes: null,
      },
    ];
  }, [activeSubTab, currentData?.cards]);

  const allCards = useMemo(() => {
    return [...(llmData?.cards ?? []), ...(nmtData?.cards ?? [])];
  }, [llmData?.cards, nmtData?.cards]);

  const cardsRef = useRef(allCards);
  cardsRef.current = allCards;
  const installedCards = useMemo(() => {
    return allCards.filter((c) => c.isInstalled);
  }, [allCards]);
  const hasInstalled = installedCards.length > 0;
  const isAnyDownloading = Object.keys(downloads).length > 0;
  const isAnyTesting = useMemo(
    () => Object.values(modelTestStates).some((s) => s.status === 'loading'),
    [modelTestStates]
  );
  const statusBadge = useMemo(() => {
    if (isAnyDownloading) {
      return (
        <span className="status-badge pending">
          <Loader2
            className="animate-spin"
            size={12}
            style={{ display: 'inline', marginRight: 4 }}
          />
          {t('settings.llm.status_downloading', { defaultValue: '正在下载' })}
        </span>
      );
    }
    if (hasInstalled) {
      return (
        <span className="status-badge ready">
          <Check size={12} style={{ display: 'inline', marginRight: 4 }} />
          {t('settings.llm.installed_ready', { defaultValue: '已就绪' })} ({installedCards.length})
        </span>
      );
    }
    return (
      <span className="status-badge off">
        {t('settings.llm.status_off', { defaultValue: '待下载模型' })}
      </span>
    );
  }, [hasInstalled, installedCards.length, isAnyDownloading, t]);

  const handleTestModel = useCallback(async (card: LocalLlmModelCardType) => {
    setModelTestStates((prev) => ({
      ...prev,
      [card.id]: {
        ...prev[card.id],
        status: 'loading',
        error: undefined,
      },
    }));

    const startTime = performance.now();
    try {
      let speed: number | undefined;
      if (card.backend === 'onnx-nmt') {
        const srcLang = card.id.replace(/^opus-mt-/, '').split('-')[0];
        const testInput =
          srcLang === 'zh'
            ? '你好，世界！'
            : srcLang === 'ja'
              ? 'こんにちは、世界！'
              : 'Hello, world!';
        const translated = await testLocalNmtModel(card.id, testInput);
        const durationSec = Math.max(0.001, (performance.now() - startTime) / 1000);
        const chars = translated ? translated.length : 0;
        speed = Math.round((chars / durationSec) * 10) / 10;
      } else {
        const res = await completeLlm({
          config: {
            provider: 'local',
            strategy: 'local',
            model: card.model || card.id,
            baseUrl: '',
            apiKey: '',
            temperature: 0.7,
          },
          input: '你好，请用一句话做自我介绍。',
          source: 'connection_test',
        });

        const durationSec = Math.max(0.001, (performance.now() - startTime) / 1000);
        const tokens =
          res.usage?.completionTokens ??
          (res.text ? Math.max(1, Math.round(res.text.length * 0.75)) : 0);
        speed = Math.round((tokens / durationSec) * 10) / 10;
      }

      setModelTestStates((prev) => ({
        ...prev,
        [card.id]: {
          status: 'success',
          speed,
        },
      }));

      setTimeout(() => {
        setModelTestStates((prev) => {
          if (prev[card.id]?.status !== 'success') return prev;
          return {
            ...prev,
            [card.id]: {
              ...prev[card.id],
              status: 'idle',
            },
          };
        });
      }, 3500);
    } catch (err) {
      const message = normalizeError(err).message;
      setModelTestStates((prev) => ({
        ...prev,
        [card.id]: {
          status: 'error',
          error: message,
        },
      }));
    }
  }, []);

  return (
    <SettingsAccordion
      title={
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span>{t('settings.llm_providers.local', { defaultValue: '本地模型' })}</span>
          <span className="local-model-badge local-model-badge-rec" style={{ fontSize: '0.7rem' }}>
            {t('settings.llm.local_engine_badge', { defaultValue: '离线端侧' })}
          </span>
        </div>
      }
      status={statusBadge}
      isOpen={isOpen}
      onToggle={onToggle}
      contentTestId="provider-accordion-content-local"
    >
      <div className="local-model-cards-container">
        {/* SubTabs and Actions */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: '16px',
          }}
        >
          <SegmentedSubTabs<'llm' | 'nmt'>
            items={[
              {
                value: 'llm',
                label: t('settings.llm.subtab_llm', { defaultValue: '大语言模型 (LLM)' }),
              },
              {
                value: 'nmt',
                label: t('settings.llm.subtab_nmt', { defaultValue: '翻译模型 (NMT)' }),
              },
            ]}
            activeTab={activeSubTab}
            onChange={setActiveSubTab}
            idPrefix="local-model-subtab"
          />
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            {activeSubTab === 'llm' && (
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={handleImportCustomFile}
                data-tooltip={t('settings.llm.import_custom_model_hint', {
                  defaultValue: '导入本地已有的 GGUF 格式模型文件',
                })}
                data-tooltip-pos="top"
              >
                <Plus size={14} />
                <span>
                  {t('settings.llm.import_custom_model', { defaultValue: '导入本地 GGUF' })}
                </span>
              </button>
            )}
            <button
              type="button"
              className="btn btn-icon btn-secondary-soft btn-sm"
              onClick={fetchCards}
              disabled={isLoading}
              data-tooltip={t('settings.llm.refresh_local_models', { defaultValue: '刷新' })}
              data-tooltip-pos="top"
              aria-label={t('settings.llm.refresh_local_models', { defaultValue: '刷新' })}
            >
              <RefreshCw size={14} className={isLoading ? 'animate-spin' : ''} />
            </button>
          </div>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {cards.map((card) => (
            <LocalModelCard
              key={card.id}
              card={card}
              downloadState={downloads[card.id]}
              testState={modelTestStates[card.id]}
              disabled={inFlightActionIds.has(card.id) || !!downloads[card.id]}
              onTest={handleTestModel}
              isAnyTesting={isAnyTesting}
              onDownload={handleDownload}
              onCancelDownload={handleCancelDownload}
              onDelete={handleDelete}
              onDiscardDownload={handleDiscardDownload}
              t={t}
            />
          ))}
        </div>
      </div>
    </SettingsAccordion>
  );
}
