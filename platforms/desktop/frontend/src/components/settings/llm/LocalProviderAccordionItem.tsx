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
  cancelDownload,
  cleanInterruptedDownload,
  deletePresetModel,
  downloadPresetModel,
} from '../../../services/tauri/app';
import { TauriEvent } from '../../../services/tauri/events';
import { completeLlm, importLocalLlmFile, listLocalLlmCards } from '../../../services/tauri/llm';
import { openDialog } from '../../../services/tauri/platform/dialog';
import { listen } from '../../../services/tauri/platform/events';
import { useConfigStore } from '../../../stores/configStore';
import { useDialogStore } from '../../../stores/dialogStore';
import { useModelDownloadStore } from '../../../stores/modelDownloadStore';
import type { LlmAssistantConfig } from '../../../types/config';
import { normalizeError } from '../../../utils/errorUtils';
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
  const [data, setData] = useState<LocalLlmCardsResponse>({ modelsDir: '', cards: [] });
  const [isLoading, setIsLoading] = useState(false);
  const [downloads, setDownloads] = useState<Record<string, LocalDownloadProgressState>>({});
  const [modelTestStates, setModelTestStates] = useState<Record<string, LocalModelTestState>>({});
  const activeDownloadsRef = useRef(downloads);
  activeDownloadsRef.current = downloads;

  const confirm = useDialogStore((state) => state.confirm);
  const alert = useDialogStore((state) => state.alert);
  const checkboxConfirm = useDialogStore((state) => state.checkboxConfirm);

  const fetchCards = useCallback(async () => {
    setIsLoading(true);
    try {
      const response = await listLocalLlmCards();
      if (response && Array.isArray(response.cards)) {
        setData(response);
      }
    } catch (err) {
      console.error('Failed to list local LLM cards:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

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

  const handleDownload = useCallback(
    async (card: LocalLlmModelCardType) => {
      const downloadId = card.id;
      if (activeDownloadsRef.current[downloadId]) return;

      if (card.interruptedDownload) {
        const { downloadedBytes, totalBytes, percentage } = card.interruptedDownload;
        const downloadedFormatted = formatBytes(downloadedBytes);
        const totalFormatted = totalBytes ? formatBytes(totalBytes) : '';
        const sizeText = totalFormatted
          ? `${downloadedFormatted} / ${totalFormatted}`
          : downloadedFormatted;
        const pctText = percentage != null ? `${percentage}%` : '';
        const progressDetail = pctText ? `${sizeText} (${pctText})` : sizeText;

        const result = await checkboxConfirm(
          t('settings.resume_confirm_message', {
            name: card.name,
            progress: progressDetail,
            defaultValue: `检测到 ${card.name} 存在未完成的下载（已完成 ${progressDetail}）。是否从断点继续下载？`,
          }),
          {
            title: t('settings.resume_confirm_title', { defaultValue: '继续下载模型' }),
            confirmLabel: t('settings.resume_download', { defaultValue: '继续下载' }),
            cancelLabel: t('common.cancel', { defaultValue: '取消' }),
            variant: 'info',
            checkbox: {
              label: t('settings.resume_restart_checkbox', {
                defaultValue: '重新开始下载（丢弃未完成的文件）',
              }),
              checkedConfirmLabel: t('settings.resume_restart_button', {
                defaultValue: '重新下载',
              }),
            },
          }
        );

        if (!result.confirmed) {
          return;
        }

        if (result.checked) {
          try {
            const cleaned = await cleanInterruptedDownload(card.id);
            if (!cleaned) {
              console.warn(`Failed to clean interrupted download for ${card.id}`);
              await alert(
                t('errors.model.clean_failed', {
                  defaultValue: '无法清理未完成的下载文件，请检查文件是否被占用。',
                })
              );
              return;
            }
            await fetchCards();
          } catch (cleanErr) {
            console.warn('Failed to clean interrupted download:', cleanErr);
            await alert(
              t('errors.model.clean_failed', {
                defaultValue: '无法清理未完成的下载文件，请检查文件是否被占用。',
              })
            );
            return;
          }
        }
      }

      useModelDownloadStore.getState().startDownload({
        modelId: card.id,
        modelName: card.name,
        modelType: 'llm',
        downloadId,
        onRetry: () => {
          void handleDownload(card);
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
        setDownloads((prev) => {
          const next = { ...prev };
          delete next[downloadId];
          return next;
        });
      }

      useModelDownloadStore.getState().completeDownload(card.id, downloadId);
      try {
        await fetchCards();
      } catch (cardErr) {
        console.warn('Failed to refresh cards after download:', cardErr);
      }
    },
    [alert, checkboxConfirm, fetchCards, modelDownloadMirror, t]
  );

  const handleCancelDownload = useCallback(
    async (cardId: string) => {
      try {
        void useModelDownloadStore.getState().cancelDownload(cardId);
        await cancelDownload(cardId);
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

  const cards = useMemo(() => {
    if (data?.cards && data.cards.length > 0) {
      return data.cards;
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
  }, [data?.cards]);

  const installedCards = useMemo(() => cards.filter((c) => c.isInstalled), [cards]);
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
      const speed = Math.round((tokens / durationSec) * 10) / 10;

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
        {/* Actions bar */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            alignItems: 'center',
            marginBottom: '8px',
            gap: '8px',
          }}
        >
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
            <span>{t('settings.llm.import_custom_model', { defaultValue: '导入本地 GGUF' })}</span>
          </button>
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
        {/* Model Cards List */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {cards.map((card) => (
            <LocalModelCard
              key={card.id}
              card={card}
              downloadState={downloads[card.id]}
              testState={modelTestStates[card.id]}
              onTest={handleTestModel}
              isAnyTesting={isAnyTesting}
              onDownload={handleDownload}
              onCancelDownload={handleCancelDownload}
              onDelete={handleDelete}
              t={t}
            />
          ))}
        </div>
      </div>
    </SettingsAccordion>
  );
}
