import type { TFunction } from 'i18next';
import type {
  ResolvedSummaryTemplate,
  SummaryCustomTemplate,
  SummaryTemplateId,
} from '../types/transcript';
import { DEFAULT_SUMMARY_TEMPLATE_ID } from '../types/transcript';

export interface BuiltInSummaryTemplate {
  id: SummaryTemplateId;
  labelKey: string;
  defaultLabel: string;
  instructions: string;
}

export const BUILTIN_SUMMARY_TEMPLATES = [
  {
    id: 'general',
    labelKey: 'summary.templates.general',
    defaultLabel: 'General',
    instructions:
      '1. A short overview paragraph.\n'
      + '2. A concise list of key points.\n'
      + '3. Follow-up items or next steps only if they are supported by the transcript.',
  },
  {
    id: 'meeting',
    labelKey: 'summary.templates.meeting',
    defaultLabel: 'Meeting',
    instructions:
      '1. Meeting overview.\n'
      + '2. Decisions made.\n'
      + '3. Action items with owners when the transcript names them.\n'
      + '4. Open questions, blockers, or risks.',
  },
  {
    id: 'lecture',
    labelKey: 'summary.templates.lecture',
    defaultLabel: 'Lecture',
    instructions:
      '1. Lecture overview.\n'
      + '2. Core concepts or arguments.\n'
      + '3. Important examples, evidence, or explanations.\n'
      + '4. Review points or next steps for study.',
  },
] as const satisfies readonly BuiltInSummaryTemplate[];

export type BuiltInSummaryTemplateId = typeof BUILTIN_SUMMARY_TEMPLATES[number]['id'];

export function isBuiltInSummaryTemplateId(
  value: string | null | undefined,
): value is BuiltInSummaryTemplateId {
  return BUILTIN_SUMMARY_TEMPLATES.some((template) => template.id === value);
}

export function getBuiltInSummaryTemplate(
  id: string | null | undefined,
): BuiltInSummaryTemplate | undefined {
  return BUILTIN_SUMMARY_TEMPLATES.find((template) => template.id === id);
}

export function normalizeSummaryCustomTemplates(
  templates: SummaryCustomTemplate[] | null | undefined,
): SummaryCustomTemplate[] {
  if (!Array.isArray(templates) || templates.length === 0) {
    return [];
  }

  const seenIds = new Set<string>();
  const normalized: SummaryCustomTemplate[] = [];

  for (let index = 0; index < templates.length; index += 1) {
    const template = templates[index];
    if (!template || typeof template !== 'object') {
      continue;
    }

    const id = typeof template.id === 'string' && template.id.trim()
      ? template.id.trim()
      : createImportedSummaryTemplateId(`${template.name ?? ''}-${template.instructions ?? ''}-${index}`);
    if (seenIds.has(id)) {
      continue;
    }

    const name = typeof template.name === 'string' ? template.name.trim() : '';
    const instructions = typeof template.instructions === 'string' ? template.instructions.trim() : '';
    if (!name || !instructions) {
      continue;
    }

    normalized.push({
      id,
      name,
      instructions,
    });
    seenIds.add(id);
  }

  return normalized;
}

export function getSummaryTemplateLabel(
  templateId: string | null | undefined,
  customTemplates: SummaryCustomTemplate[] | null | undefined,
  t: TFunction,
): string {
  const builtIn = getBuiltInSummaryTemplate(templateId);
  if (builtIn) {
    return t(builtIn.labelKey, { defaultValue: builtIn.defaultLabel });
  }

  return normalizeSummaryCustomTemplates(customTemplates).find((template) => template.id === templateId)?.name
    || t('summary.templates.general', { defaultValue: 'General' });
}

export function resolveSummaryTemplate(
  templateId: string | null | undefined,
  customTemplates: SummaryCustomTemplate[] | null | undefined,
  t?: TFunction,
): ResolvedSummaryTemplate {
  const builtIn = getBuiltInSummaryTemplate(templateId);
  if (builtIn) {
    return {
      id: builtIn.id,
      name: t ? t(builtIn.labelKey, { defaultValue: builtIn.defaultLabel }) : builtIn.defaultLabel,
      instructions: builtIn.instructions,
      builtIn: true,
    };
  }

  const custom = normalizeSummaryCustomTemplates(customTemplates).find((template) => template.id === templateId);
  if (custom) {
    return {
      id: custom.id,
      name: custom.name,
      instructions: custom.instructions,
      builtIn: false,
    };
  }

  const fallback = getBuiltInSummaryTemplate(DEFAULT_SUMMARY_TEMPLATE_ID)!;
  return {
    id: fallback.id,
    name: t ? t(fallback.labelKey, { defaultValue: fallback.defaultLabel }) : fallback.defaultLabel,
    instructions: fallback.instructions,
    builtIn: true,
  };
}

export function getSummaryTemplateOptions(
  customTemplates: SummaryCustomTemplate[] | null | undefined,
  t: TFunction,
): Array<{ value: string; label: string }> {
  const builtInOptions = BUILTIN_SUMMARY_TEMPLATES.map((template) => ({
    value: template.id,
    label: t(template.labelKey, { defaultValue: template.defaultLabel }),
  }));
  const customOptions = normalizeSummaryCustomTemplates(customTemplates).map((template) => ({
    value: template.id,
    label: template.name,
  }));

  return [...builtInOptions, ...customOptions];
}

export function coerceSummaryTemplateId(
  templateId: string | null | undefined,
  customTemplates: SummaryCustomTemplate[] | null | undefined,
): SummaryTemplateId {
  if (isBuiltInSummaryTemplateId(templateId)) {
    return templateId;
  }

  return normalizeSummaryCustomTemplates(customTemplates).some((template) => template.id === templateId)
    ? (templateId as SummaryTemplateId)
    : DEFAULT_SUMMARY_TEMPLATE_ID;
}

function createImportedSummaryTemplateId(seed: string): string {
  return `summary-template-${hashString(seed)}`;
}

function hashString(value: string): string {
  let hash = 5381;
  for (let index = 0; index < value.length; index += 1) {
    hash = ((hash << 5) + hash) ^ value.charCodeAt(index);
  }

  return Math.abs(hash >>> 0).toString(16).padStart(8, '0');
}
