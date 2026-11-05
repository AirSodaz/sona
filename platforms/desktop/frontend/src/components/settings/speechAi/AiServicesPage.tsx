import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { getProviderDefinition } from '../../../services/llm/providers';
import { getOrderedLlmModels } from '../../../services/llm/state';
import { useLlmAssistantConfig } from '../../../stores/configStore';
import type { LlmModelEntry, LlmProvider } from '../../../types/transcript';
import { getCurrentLlmSettings } from '../llm/helpers';
import { isGoogleProvider, isLocalLlmProvider } from './speechAiLogic';
import { useLlmProbe } from './useLlmProbe';

export function AiServicesPage({
  onAdd,
  onEdit,
}: {
  onAdd: () => void;
  onEdit: (provider: LlmProvider) => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const llmConfig = useLlmAssistantConfig();
  const settings = useMemo(() => getCurrentLlmSettings(llmConfig), [llmConfig]);
  const services = useMemo(() => {
    const seen = new Set<string>();
    const rows: LlmModelEntry[] = [];
    for (const entry of getOrderedLlmModels(settings)) {
      if (isGoogleProvider(entry.provider) || seen.has(entry.provider)) continue;
      seen.add(entry.provider);
      rows.push(entry);
    }
    return rows;
  }, [settings]);

  return (
    <div className="speech-ai" id="settings-speech-ai-services">
      <p className="settings-page-description">{t('settings.speech_ai.ai_page_desc')}</p>
      <div className="speech-ai-card" id="settings-llm-provider-list">
        {services.map((entry) => (
          <ServiceRow
            key={entry.provider}
            entry={entry}
            name={providerName(entry.provider, settings.customProviders, t)}
            onOpen={() => onEdit(entry.provider)}
          />
        ))}
      </div>
      <button
        type="button"
        id="settings-llm-custom-provider"
        className={services.length === 0 ? 'btn btn-primary' : 'btn btn-secondary'}
        onClick={onAdd}
      >
        {t('settings.speech_ai.ai_add')}
      </button>
    </div>
  );
}

function ServiceRow({
  entry,
  name,
  onOpen,
}: {
  entry: LlmModelEntry;
  name: string;
  onOpen: () => void;
}): React.JSX.Element {
  const { t } = useTranslation();
  const local = isLocalLlmProvider(entry.provider);
  const probe = useLlmProbe(entry.provider, entry.model, true);
  const tone = probe.busy ? 'busy' : probe.ok ? 'ok' : 'err';
  const label = probe.busy
    ? t('settings.speech_ai.ai_checking')
    : probe.ok
      ? local
        ? t('settings.speech_ai.ai_local')
        : t('settings.speech_ai.ai_connected')
      : local
        ? t('settings.speech_ai.ai_not_running')
        : t('settings.speech_ai.ai_unreachable');
  const hint = local ? t('settings.speech_ai.ai_local_hint', { model: entry.model }) : entry.model;

  return (
    <button
      type="button"
      className="speech-ai-drill"
      onClick={onOpen}
      aria-label={`${name}, ${label}`}
    >
      <span className="settings-item-info">
        <span className="settings-item-title">{name}</span>
        <span className="settings-item-hint">{hint}</span>
      </span>
      <span className={`speech-ai-drill-value is-${tone}`}>
        <span className="speech-ai-dot" aria-hidden="true" />
        <span>{label}</span>
        <span aria-hidden="true">›</span>
      </span>
    </button>
  );
}

function providerName(
  provider: LlmProvider,
  customProviders: ReturnType<typeof getCurrentLlmSettings>['customProviders'],
  t: (key: string) => string
): string {
  const custom = customProviders?.[provider as `custom-${string}`]?.name;
  if (custom) return custom;
  return t(getProviderDefinition(provider, customProviders).labelKey);
}
