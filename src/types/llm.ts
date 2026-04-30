export type LlmProvider =
  | 'open_ai'
  | 'open_ai_responses'
  | 'azure_openai'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'deep_seek'
  | 'kimi'
  | 'silicon_flow'
  | 'qwen'
  | 'qwen_portal'
  | 'minimax_global'
  | 'minimax_cn'
  | 'openrouter'
  | 'lm_studio'
  | 'groq'
  | 'x_ai'
  | 'mistral_ai'
  | 'perplexity'
  | 'volcengine'
  | 'chatglm'
  | 'google_translate'
  | 'google_translate_free'
  | 'open_ai_compatible';

export type LlmProviderStrategy =
  | 'openai_compatible'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'azure_openai'
  | 'openai_responses'
  | 'openai_compatible_custom_path'
  | 'google_translate'
  | 'google_translate_free'
  | 'perplexity';

export type LlmFeature = 'polish' | 'translation' | 'summary';

export interface LlmProviderSetting {
  /** Provider API host or endpoint. */
  apiHost: string;
  /** Provider API key or token. */
  apiKey: string;
  /** Optional provider-specific API path override. */
  apiPath?: string;
  /** Optional provider-specific API version. */
  apiVersion?: string;
}

export interface LlmModelEntry {
  /** Stable identifier for the configured model entry. */
  id: string;
  /** Provider used for this model entry. */
  provider: LlmProvider;
  /** Model name or deployment name. */
  model: string;
}

export interface LlmFeatureSelections {
  /** Selected model entry for polish. */
  polishModelId?: string;
  /** Selected model entry for translation. */
  translationModelId?: string;
  /** Selected model entry for summary. */
  summaryModelId?: string;
  /** Temperature override for polish. */
  polishTemperature?: number;
  /** Temperature override for translation. */
  translationTemperature?: number;
  /** Temperature override for summary. */
  summaryTemperature?: number;
}

export interface LlmSettings {
  /** Currently active provider. */
  activeProvider: LlmProvider;
  /** Per-provider saved settings. */
  providers: Partial<Record<LlmProvider, LlmProviderSetting>>;
  /** Added models keyed by ID. */
  models: Record<string, LlmModelEntry>;
  /** Ordered list of added model IDs. */
  modelOrder: string[];
  /** Feature-specific model selections. */
  selections: LlmFeatureSelections;
}

export interface LlmConfig {
  /** LLM provider kind. */
  provider: LlmProvider;
  /** LLM base URL. */
  baseUrl: string;
  /** LLM API key. */
  apiKey: string;
  /** LLM model name. */
  model: string;
  /** Optional provider-specific path. */
  apiPath?: string;
  /** Optional provider-specific API version. */
  apiVersion?: string;
  /** LLM temperature (0.0 to 2.0). */
  temperature?: number;
}

export type SummaryTemplateId = string;

export interface SummaryCustomTemplate {
  /** Stable template id. */
  id: string;
  /** User-visible template name. */
  name: string;
  /** Template instructions sent to the summary prompt. */
  instructions: string;
}

export interface ResolvedSummaryTemplate {
  /** Stable template id. */
  id: SummaryTemplateId;
  /** Resolved display name. */
  name: string;
  /** Instructions sent to the prompt builder. */
  instructions: string;
  /** Whether this template is built into the app. */
  builtIn: boolean;
}

export type SummaryTemplate = SummaryTemplateId;

export const DEFAULT_SUMMARY_TEMPLATE_ID: SummaryTemplateId = 'general';
export const DEFAULT_SUMMARY_TEMPLATE: SummaryTemplate = DEFAULT_SUMMARY_TEMPLATE_ID;
