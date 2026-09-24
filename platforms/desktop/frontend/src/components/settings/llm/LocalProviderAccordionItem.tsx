import { Check, Cpu, Loader2, Plus, RefreshCw, X } from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type {
  LocalLlmCardsResponse,
  LocalLlmModelCard as LocalLlmModelCardType,
} from '../../../bindings';
import {
  addLlmModel,
  getFeatureModelEntry,
  setFeatureModelSelection,
} from '../../../services/llm/state';
import { parseDownloadProgressPayload } from '../../../services/modelDownloadService';
import {
  cancelDownload,
  deletePresetModel,
  downloadPresetModel,
} from '../../../services/tauri/app';
import { TauriEvent } from '../../../services/tauri/events';
import {
  generateLlmText,
  importLocalLlmFile,
  listLocalLlmCards,
} from '../../../services/tauri/llm';
import { openDialog } from '../../../services/tauri/platform/dialog';
import { listen } from '../../../services/tauri/platform/events';
import { useConfigStore } from '../../../stores/configStore';
import { useDialogStore } from '../../../stores/dialogStore';
import type { LlmAssistantConfig } from '../../../types/config';
import type { LlmGenerateCommandRequest } from '../../../types/dashboard';
import { normalizeError } from '../../../utils/errorUtils';
import { SettingsAccordion, SettingsItem } from '../SettingsLayout';
import { getCurrentLlmState } from './helpers';
import { type LocalDownloadProgressState, LocalModelCard } from './LocalModelCard';
import './LocalModelCard.css';

interface LocalProviderAccordionItemProps {
  config: LlmAssistantConfig;
  isOpen: boolean;
  onToggle: () => void;
  applyLlmSettings: (nextSettings: LlmAssistantConfig['llmSettings']) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

export function LocalProviderAccordionItem({
  config,
  isOpen,
  onToggle,
  applyLlmSettings,
  t,
}: LocalProviderAccordionItemProps): React.JSX.Element {
  const modelDownloadMirror = useConfigStore((state) => state.config.modelDownloadMirror);
  const [data, setData] = useState<LocalLlmCardsResponse>({ modelsDir: '', cards: [] });
  const [isLoading, setIsLoading] = useState(false);
  const [downloads, setDownloads] = useState<Record<string, LocalDownloadProgressState>>({});
  const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [testMessage, setTestMessage] = useState('');
  const activeDownloadsRef = useRef(downloads);
  activeDownloadsRef.current = downloads;

  const confirm = useDialogStore((state) => state.confirm);

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
        await fetchCards();
      } catch (err) {
        console.error(`Failed to download model ${card.id}:`, err);
      } finally {
        setDownloads((prev) => {
          const next = { ...prev };
          delete next[downloadId];
          return next;
        });
      }
    },
    [fetchCards, modelDownloadMirror, t]
  );

  const handleCancelDownload = useCallback(async (cardId: string) => {
    try {
      await cancelDownload(cardId);
    } catch (err) {
      console.error(`Failed to cancel download for ${cardId}:`, err);
    } finally {
      setDownloads((prev) => {
        const next = { ...prev };
        delete next[cardId];
        return next;
      });
    }
  }, []);

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

  const currentPolish = getFeatureModelEntry(config, 'polish');
  const currentTranslation = getFeatureModelEntry(config, 'translation');
  const currentSummary = getFeatureModelEntry(config, 'summary');

  const handleApplyFeature = useCallback(
    (card: LocalLlmModelCardType, feature: 'polish' | 'translation' | 'summary' | 'all') => {
      if (!card.isInstalled) {
        return;
      }
      const currentLlmState = getCurrentLlmState(config);
      const isReasoning = card.capabilities?.includes('reasoning');
      let nextState = addLlmModel(currentLlmState.llmSettings, {
        provider: 'local',
        model: card.model,
        metadata: isReasoning
          ? {
              displayName: card.name,
              contextWindow: card.contextWindow,
              maxOutputTokens: card.maxOutputTokens,
              supportsReasoning: true,
              reasoningMode: {
                type: 'effort',
                supported_levels: [
                  { mode: 'minimal' },
                  { mode: 'low' },
                  { mode: 'medium' },
                  { mode: 'high' },
                  { mode: 'xhigh' },
                  { mode: 'max' },
                ],
              },
              supportedThinkingLevels: [
                { mode: 'minimal' },
                { mode: 'low' },
                { mode: 'medium' },
                { mode: 'high' },
                { mode: 'xhigh' },
                { mode: 'max' },
              ],
            }
          : {
              displayName: card.name,
              contextWindow: card.contextWindow,
              maxOutputTokens: card.maxOutputTokens,
              supportsReasoning: false,
            },
      });
      const entryId = nextState.modelOrder.find((id) => {
        const existing = nextState.models[id];
        return existing?.provider === 'local' && existing.model === card.model;
      });

      if (entryId) {
        if (feature === 'polish' || feature === 'all') {
          nextState = setFeatureModelSelection(nextState, 'polish', entryId);
        }
        if (feature === 'translation' || feature === 'all') {
          nextState = setFeatureModelSelection(nextState, 'translation', entryId);
        }
        if (feature === 'summary' || feature === 'all') {
          nextState = setFeatureModelSelection(nextState, 'summary', entryId);
        }
      }

      applyLlmSettings({
        ...nextState,
        activeProvider: 'local',
      });
    },
    [applyLlmSettings, config]
  );

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

  const handleTestInference = async () => {
    const firstModel = installedCards[0]?.model ?? 'Qwen/Qwen3.5-4B';
    setTestStatus('loading');
    setTestMessage('');
    try {
      const res = await generateLlmText({
        config: {
          provider: 'local',
          strategy: 'local',
          model: firstModel,
          baseUrl: '',
          apiKey: '',
          temperature: 0.7,
        },
        input: '你好，请用一句话做自我介绍。',
        source: 'connection_test',
      } satisfies LlmGenerateCommandRequest);
      setTestStatus('success');
      setTestMessage(res.trim());
      setTimeout(() => {
        setTestStatus('idle');
        setTestMessage('');
      }, 5000);
    } catch (error) {
      setTestStatus('error');
      setTestMessage(normalizeError(error).message);
    }
  };

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
          {cards.map((card) => {
            const isPolish =
              currentPolish?.provider === 'local' && currentPolish.model === card.model;
            const isTranslation =
              currentTranslation?.provider === 'local' && currentTranslation.model === card.model;
            const isSummary =
              currentSummary?.provider === 'local' && currentSummary.model === card.model;

            return (
              <LocalModelCard
                key={card.id}
                card={card}
                downloadState={downloads[card.id]}
                activeFeatures={{
                  polish: isPolish,
                  translation: isTranslation,
                  summary: isSummary,
                }}
                onDownload={handleDownload}
                onCancelDownload={handleCancelDownload}
                onDelete={handleDelete}
                onApplyFeature={handleApplyFeature}
                t={t}
              />
            );
          })}
        </div>

        {/* Inference Connection Test */}
        {hasInstalled && (
          <SettingsItem
            title={t('settings.llm.test_connection_title', { defaultValue: '推理测试' })}
            hint={t('settings.llm.test_connection_hint', {
              defaultValue: '向本地模型发送测试请求以验证引擎生成状态',
            })}
          >
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'flex-end',
                gap: '8px',
              }}
            >
              <button
                type="button"
                className={`btn ${
                  testStatus === 'success'
                    ? 'btn-success-flash'
                    : testStatus === 'error'
                      ? 'btn-error-flash'
                      : 'btn-secondary'
                } btn-loading-wrapper`}
                style={{ width: 'fit-content', minWidth: '130px' }}
                onClick={handleTestInference}
                disabled={testStatus === 'loading'}
              >
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    gap: '8px',
                  }}
                >
                  {testStatus === 'loading' ? (
                    <Loader2 className="animate-spin" size={16} />
                  ) : testStatus === 'success' ? (
                    <Check size={16} />
                  ) : testStatus === 'error' ? (
                    <X size={16} />
                  ) : (
                    <Cpu size={16} />
                  )}
                  <span>
                    {testStatus === 'loading'
                      ? t('settings.llm.testing', { defaultValue: '正在生成...' })
                      : testStatus === 'success'
                        ? t('settings.llm.connection_success', { defaultValue: '推理成功' })
                        : testStatus === 'error'
                          ? t('settings.llm.connection_failed', { defaultValue: '测试失败' })
                          : t('settings.llm.test_connection', { defaultValue: '运行推理测试' })}
                  </span>
                </div>
              </button>

              {testMessage && (
                <div
                  className={
                    testStatus === 'error' ? 'connection-error-detail' : 'connection-success-detail'
                  }
                  style={{
                    fontSize: '0.8rem',
                    color: testStatus === 'error' ? 'var(--color-error)' : 'var(--color-success)',
                    maxWidth: '400px',
                    textAlign: 'right',
                  }}
                >
                  <span>{testMessage}</span>
                </div>
              )}
            </div>
          </SettingsItem>
        )}
      </div>
    </SettingsAccordion>
  );
}
