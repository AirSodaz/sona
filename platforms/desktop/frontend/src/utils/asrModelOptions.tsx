import type { DropdownOption } from '../components/Dropdown';
import { OnlineIcon } from '../components/Icons';
import { ModelBrandLogo } from '../components/icons/ModelLogos';
import {
  getProviderAddedModels,
  ONLINE_ASR_PROVIDER_DEFAULT_NAMES,
  ONLINE_ASR_PROVIDER_DEFINITIONS,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
} from '../services/onlineAsrProviders';
import type { AsrSelectionSlot, ModelConfig } from '../types/config';
import type { ModelCatalogSnapshot, ModelSelectionOption } from '../types/modelCatalog';

function resolveSelectedModelIdFromPath(
  snapshot: Pick<ModelCatalogSnapshot, 'modelIdByNormalizedPath' | 'pathMatchTokens'>,
  modelPath: string,
  options: Array<{ id: string }>
): string | null {
  if (!modelPath.trim()) {
    return null;
  }

  const normalizedPath = modelPath.replace(/\\/g, '/').toLowerCase();
  const exactModelId = snapshot.modelIdByNormalizedPath?.[normalizedPath];
  if (
    exactModelId &&
    (options.length === 0 || options.some((option) => option.id === exactModelId))
  ) {
    return exactModelId;
  }

  for (const option of options) {
    const token = snapshot.pathMatchTokens?.find((item) => item.id === option.id);
    if (token?.token && normalizedPath.includes(token.token.toLowerCase())) {
      return option.id;
    }
  }

  return null;
}

/**
 * Resolves the active model ID for an ASR slot (streaming/live or batch),
 * whether online provider (with or without submodel) or local model.
 */
export function resolveActiveAsrModelId(
  config: ModelConfig,
  slot: AsrSelectionSlot,
  snapshot?: Pick<
    ModelCatalogSnapshot,
    'selectionOptions' | 'modelIdByNormalizedPath' | 'pathMatchTokens'
  > | null
): string {
  const selection = config.asr?.selections?.[slot];
  if (selection?.engine === 'online') {
    const pid = selection.providerId || ONLINE_ASR_PROVIDER_DEFINITIONS[0].id;
    return selection.modelId ? `${pid}::${selection.modelId}` : pid;
  }

  const isBatch = slot === 'batch';
  const options = isBatch
    ? (snapshot?.selectionOptions?.batch ?? [])
    : (snapshot?.selectionOptions?.streaming ?? []);

  if (selection?.modelId) {
    if (options.length === 0 || options.some((opt) => opt.id === selection.modelId)) {
      return selection.modelId;
    }
  }

  const modelPath =
    selection?.modelPath?.trim() ||
    (isBatch ? config.batchModelPath : config.streamingModelPath) ||
    '';

  if (!modelPath || !snapshot) {
    return selection?.modelId ?? '';
  }

  return resolveSelectedModelIdFromPath(snapshot, modelPath, options) ?? selection?.modelId ?? '';
}

/**
 * Maps model selection options to DropdownOption format, filtering by installed status.
 */
export function toDropdownOptions(
  options: ModelSelectionOption[],
  selectedId: string,
  withIcon = false
): DropdownOption[] {
  return options
    .filter((option) => option.isInstalled || option.id === selectedId)
    .map((option) => ({
      value: option.id,
      ariaLabel: option.label,
      label: withIcon ? (
        <span className="model-dropdown-option">
          <span className="model-dropdown-option-icon">
            <ModelBrandLogo
              model={{ id: option.id, name: option.label }}
              size={16}
              alt=""
              aria-hidden="true"
            />
          </span>
          <span className="model-dropdown-option-label">{option.label}</span>
        </span>
      ) : (
        option.label
      ),
    }));
}

export interface BuildAsrModelDropdownOptionsParams {
  slot: AsrSelectionSlot;
  snapshot: ModelCatalogSnapshot;
  selectedModelId: string;
  modelConfig: ModelConfig;
  t: (key: string, options?: Record<string, unknown>) => string;
}

/**
 * Builds ASR model dropdown options including local installed models and configured online providers.
 */
export function buildAsrModelDropdownOptions({
  slot,
  snapshot,
  selectedModelId,
  modelConfig,
  t,
}: BuildAsrModelDropdownOptionsParams): DropdownOption[] {
  const isBatch = slot === 'batch';
  const mode = isBatch ? 'batch' : 'streaming';

  const rawLocalOptions = isBatch
    ? (snapshot.selectionOptions?.batch ?? [])
    : (snapshot.selectionOptions?.streaming ?? []);

  const localOptions = toDropdownOptions(rawLocalOptions, selectedModelId, true);

  const cloudOptions: DropdownOption[] = [];

  for (const provider of ONLINE_ASR_PROVIDER_DEFINITIONS) {
    if (isBatch) {
      if (provider.manifestEntry.batch?.localFileMode?.supported === false) continue;
    } else {
      if (provider.manifestEntry.streaming?.supported === false) continue;
    }

    const providerConfig =
      modelConfig.asr?.providers?.online?.[provider.id] ??
      (provider.id === VOLCENGINE_DOUBAO_PROVIDER_ID
        ? modelConfig.asr?.providers?.volcengineDoubao
        : undefined) ??
      provider.defaultConfig;

    const isConfigured = provider.isConfigured(
      providerConfig as typeof provider.defaultConfig,
      mode
    );
    const isSelected =
      selectedModelId === provider.id || selectedModelId?.startsWith(`${provider.id}::`);
    if (!isConfigured && !isSelected) continue;

    const addedModels = getProviderAddedModels(providerConfig, provider);
    const modeModels = provider.models.filter((m) => m.modes.includes(mode));
    const activeModels = modeModels.filter(
      (m) =>
        addedModels.includes(m.id) ||
        selectedModelId === `${provider.id}::${m.id}` ||
        (selectedModelId === provider.id && m.isDefault)
    );

    const providerLabel = t(provider.optionLabelKey, {
      defaultValue: ONLINE_ASR_PROVIDER_DEFAULT_NAMES[provider.id] ?? provider.id,
    });

    if (activeModels.length === 0) {
      if (isSelected) {
        cloudOptions.push({
          value: provider.id,
          ariaLabel: providerLabel,
          label: (
            <span className="model-dropdown-option">
              <span className="model-dropdown-option-icon">
                <ModelBrandLogo
                  model={{ id: provider.id, name: providerLabel }}
                  size={16}
                  alt=""
                  aria-hidden="true"
                />
              </span>
              <span className="model-dropdown-option-label">{providerLabel}</span>
              <OnlineIcon className="model-dropdown-cloud-icon" width={14} height={14} />
            </span>
          ),
        });
      }
    } else {
      for (const model of activeModels) {
        const optionValue = activeModels.length > 1 ? `${provider.id}::${model.id}` : provider.id;
        const optionLabel = `${model.name} · ${providerLabel}`;
        cloudOptions.push({
          value: optionValue,
          ariaLabel: optionLabel,
          label: (
            <span className="model-dropdown-option">
              <span className="model-dropdown-option-icon">
                <ModelBrandLogo
                  model={{ id: provider.id, name: providerLabel }}
                  size={16}
                  alt=""
                  aria-hidden="true"
                />
              </span>
              <span className="model-dropdown-option-label">{optionLabel}</span>
              <OnlineIcon className="model-dropdown-cloud-icon" width={14} height={14} />
            </span>
          ),
        });
      }
    }
  }

  return [...localOptions, ...cloudOptions];
}
