import React, { useState } from 'react';
import { X } from 'lucide-react';
import type { LlmModelEntry, LlmModelMetadata, LlmModality } from '../../../types/transcript';

type ModelMetadataDraft = {
  displayName: string;
  contextWindow: string;
  maxOutputTokens: string;
  inputPrice: string;
  outputPrice: string;
  cacheReadPrice: string;
  cacheWritePrice: string;
  knowledgeCutoff: string;
  releaseDate: string;
  lastUpdated: string;
  inputModalities: LlmModality[];
  outputModalities: LlmModality[];
  supportsMultimodal: boolean;
  supportsTools: boolean;
  supportsReasoning: boolean;
  supportsStructuredOutput: boolean;
  supportsPromptCaching: boolean;
};

type ModelMetadataEditorProps = {
  entry: LlmModelEntry;
  onCancel: () => void;
  onSave: (metadata: Partial<LlmModelMetadata>) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
};

const MODEL_MODALITIES: LlmModality[] = ['text', 'image', 'audio', 'video', 'pdf'];

function formatDraftNumber(value: number | undefined): string {
  return typeof value === 'number' ? String(value) : '';
}

function createDraft(entry: LlmModelEntry): ModelMetadataDraft {
  return {
    displayName: entry.metadata?.displayName ?? '',
    contextWindow: formatDraftNumber(entry.metadata?.contextWindow),
    maxOutputTokens: formatDraftNumber(entry.metadata?.maxOutputTokens),
    inputPrice: formatDraftNumber(entry.metadata?.inputPrice),
    outputPrice: formatDraftNumber(entry.metadata?.outputPrice),
    cacheReadPrice: formatDraftNumber(entry.metadata?.cacheReadPrice),
    cacheWritePrice: formatDraftNumber(entry.metadata?.cacheWritePrice),
    knowledgeCutoff: entry.metadata?.knowledgeCutoff ?? '',
    releaseDate: entry.metadata?.releaseDate ?? '',
    lastUpdated: entry.metadata?.lastUpdated ?? '',
    inputModalities: [...(entry.metadata?.inputModalities ?? [])],
    outputModalities: [...(entry.metadata?.outputModalities ?? [])],
    supportsMultimodal: Boolean(entry.metadata?.supportsMultimodal),
    supportsTools: Boolean(entry.metadata?.supportsTools),
    supportsReasoning: Boolean(entry.metadata?.supportsReasoning),
    supportsStructuredOutput: Boolean(entry.metadata?.supportsStructuredOutput),
    supportsPromptCaching: Boolean(entry.metadata?.supportsPromptCaching),
  };
}

function parseOptionalNonNegativeNumber(value: string): number | undefined | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function parseOptionalText(value: string): string | undefined {
  return value.trim() || undefined;
}

export const ModelMetadataEditor = React.memo(function ModelMetadataEditor({
  entry,
  onCancel,
  onSave,
  t,
}: ModelMetadataEditorProps) {
  const [draft, setDraft] = useState<ModelMetadataDraft>(() => createDraft(entry));
  const [dirtyFields, setDirtyFields] = useState<Set<keyof ModelMetadataDraft>>(() => new Set());
  const [error, setError] = useState('');

  const markDirty = (field: keyof ModelMetadataDraft) => {
    setDirtyFields((current) => current.has(field) ? current : new Set(current).add(field));
  };

  const setText = (
    field: keyof Pick<ModelMetadataDraft, 'displayName' | 'knowledgeCutoff' | 'releaseDate' | 'lastUpdated'>,
    value: string,
  ) => {
    setDraft((current) => ({ ...current, [field]: value }));
    markDirty(field);
  };

  const setNumber = (
    field: keyof Pick<ModelMetadataDraft, 'contextWindow' | 'maxOutputTokens' | 'inputPrice' | 'outputPrice' | 'cacheReadPrice' | 'cacheWritePrice'>,
    value: string,
  ) => {
    setDraft((current) => ({ ...current, [field]: value }));
    markDirty(field);
    setError('');
  };

  const setCapability = (
    field: keyof Pick<ModelMetadataDraft, 'supportsMultimodal' | 'supportsTools' | 'supportsReasoning' | 'supportsStructuredOutput' | 'supportsPromptCaching'>,
    checked: boolean,
  ) => {
    setDraft((current) => ({ ...current, [field]: checked }));
    markDirty(field);
  };

  const setModality = (
    field: 'inputModalities' | 'outputModalities',
    modality: LlmModality,
    checked: boolean,
  ) => {
    markDirty(field);
    setDraft((current) => ({
      ...current,
      [field]: checked
        ? [...new Set([...current[field], modality])]
        : current[field].filter((value) => value !== modality),
    }));
  };

  const save = () => {
    const contextWindow = parseOptionalNonNegativeNumber(draft.contextWindow);
    const maxOutputTokens = parseOptionalNonNegativeNumber(draft.maxOutputTokens);
    const inputPrice = parseOptionalNonNegativeNumber(draft.inputPrice);
    const outputPrice = parseOptionalNonNegativeNumber(draft.outputPrice);
    const cacheReadPrice = parseOptionalNonNegativeNumber(draft.cacheReadPrice);
    const cacheWritePrice = parseOptionalNonNegativeNumber(draft.cacheWritePrice);
    if ([
      contextWindow,
      maxOutputTokens,
      inputPrice,
      outputPrice,
      cacheReadPrice,
      cacheWritePrice,
    ].includes(null)) {
      setError(t('settings.llm.model_metadata_invalid_number'));
      return;
    }

    const metadata: Partial<LlmModelMetadata> = {
      displayName: parseOptionalText(draft.displayName),
      contextWindow: contextWindow ?? undefined,
      maxOutputTokens: maxOutputTokens ?? undefined,
      inputPrice: inputPrice ?? undefined,
      outputPrice: outputPrice ?? undefined,
      cacheReadPrice: cacheReadPrice ?? undefined,
      cacheWritePrice: cacheWritePrice ?? undefined,
      knowledgeCutoff: parseOptionalText(draft.knowledgeCutoff),
      releaseDate: parseOptionalText(draft.releaseDate),
      lastUpdated: parseOptionalText(draft.lastUpdated),
      inputModalities: draft.inputModalities,
      outputModalities: draft.outputModalities,
      supportsMultimodal: draft.supportsMultimodal,
      supportsTools: draft.supportsTools,
      supportsReasoning: draft.supportsReasoning,
      supportsStructuredOutput: draft.supportsStructuredOutput,
      supportsPromptCaching: draft.supportsPromptCaching,
    };
    const changes: Partial<LlmModelMetadata> = {};
    dirtyFields.forEach((field) => {
      changes[field] = metadata[field] as never;
    });
    onSave(changes);
  };

  const textFields = [
    ['displayName', 'settings.llm.model_display_name', true],
    ['knowledgeCutoff', 'settings.llm.model_knowledge_cutoff', false],
    ['releaseDate', 'settings.llm.model_release_date', false],
    ['lastUpdated', 'settings.llm.model_last_updated', false],
  ] as const;
  const numberFields = [
    ['contextWindow', 'settings.llm.model_context_window', false],
    ['maxOutputTokens', 'settings.llm.model_max_output_tokens', false],
    ['inputPrice', 'settings.llm.model_input_price', true],
    ['outputPrice', 'settings.llm.model_output_price', true],
    ['cacheReadPrice', 'settings.llm.model_cache_read_price', true],
    ['cacheWritePrice', 'settings.llm.model_cache_write_price', true],
  ] as const;
  const capabilities = [
    ['supportsMultimodal', 'settings.llm.model_supports_multimodal'],
    ['supportsTools', 'settings.llm.model_supports_tools'],
    ['supportsReasoning', 'settings.llm.model_supports_reasoning'],
    ['supportsStructuredOutput', 'settings.llm.model_supports_structured_output'],
    ['supportsPromptCaching', 'settings.llm.model_supports_prompt_caching'],
  ] as const;

  return (
    <div className="provider-model-metadata-editor">
      {error ? (
        <div className="connection-error-detail">
          <X size={12} />
          <span>{error}</span>
        </div>
      ) : null}
      <div className="provider-model-metadata-editor-grid">
        {textFields.map(([field, label, wide]) => (
          <label
            className={`provider-model-metadata-field${wide ? ' provider-model-metadata-field-wide' : ''}`}
            key={field}
          >
            <span className="settings-label">{t(label)}</span>
            <input
              className="settings-input"
              type="text"
              value={draft[field]}
              onChange={(event) => setText(field, event.target.value)}
            />
          </label>
        ))}
        {numberFields.map(([field, label, decimal]) => (
          <label className="provider-model-metadata-field" key={field}>
            <span className="settings-label">{t(label)}</span>
            <input
              className="settings-input"
              type="number"
              min={0}
              step={decimal ? 'any' : undefined}
              value={draft[field]}
              onChange={(event) => setNumber(field, event.target.value)}
            />
          </label>
        ))}
      </div>
      <div className="provider-model-modality-editor">
        {(['inputModalities', 'outputModalities'] as const).map((field) => (
          <fieldset className="provider-model-modality-group" key={field}>
            <legend className="settings-label">
              {t(field === 'inputModalities'
                ? 'settings.llm.model_input_modalities'
                : 'settings.llm.model_output_modalities')}
            </legend>
            <div className="provider-model-modality-options">
              {MODEL_MODALITIES.map((modality) => (
                <label className="provider-model-checkbox" key={modality}>
                  <input
                    type="checkbox"
                    checked={draft[field].includes(modality)}
                    onChange={(event) => setModality(field, modality, event.target.checked)}
                  />
                  <span>{t(`settings.llm.modality_${modality}`)}</span>
                </label>
              ))}
            </div>
          </fieldset>
        ))}
      </div>
      <div className="provider-model-capability-editor">
        {capabilities.map(([field, label]) => (
          <label className="provider-model-checkbox" key={field}>
            <input
              type="checkbox"
              checked={draft[field]}
              onChange={(event) => setCapability(field, event.target.checked)}
            />
            <span>{t(label)}</span>
          </label>
        ))}
      </div>
      <div className="provider-model-editor-actions">
        <button type="button" className="btn btn-primary" onClick={save}>
          {t('settings.llm.save_model_metadata')}
        </button>
        <button type="button" className="btn btn-secondary" onClick={onCancel}>
          {t('common.cancel')}
        </button>
      </div>
    </div>
  );
});
