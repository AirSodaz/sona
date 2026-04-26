import type { TFunction } from 'i18next';
import type { PolishCustomPreset } from '../types/config';

export interface BuiltInPolishPreset {
  id: string;
  labelKey: string;
  defaultLabel: string;
  context: string;
}

export interface ResolvedPolishPreset {
  id: string;
  name: string;
  context: string;
  builtIn: boolean;
}

export interface LegacyPolishSelectionInput {
  presetId?: string | null;
  scenario?: string | null;
  context?: string | null;
}

export interface LegacyPolishSelectionResult {
  presetId: string;
  customPresets: PolishCustomPreset[];
}

export const BUILTIN_POLISH_PRESETS = [
  {
    id: 'general',
    labelKey: 'polish.scenarios.general',
    defaultLabel: 'General',
    context: '',
  },
  {
    id: 'customer_service',
    labelKey: 'polish.scenarios.customer_service',
    defaultLabel: 'Customer Service Call',
    context: 'This is a transcript of a customer service call.',
  },
  {
    id: 'meeting',
    labelKey: 'polish.scenarios.meeting',
    defaultLabel: 'Meeting',
    context: 'This is a transcript of a meeting.',
  },
  {
    id: 'interview',
    labelKey: 'polish.scenarios.interview',
    defaultLabel: 'Interview',
    context: 'This is a transcript of an interview.',
  },
  {
    id: 'lecture',
    labelKey: 'polish.scenarios.lecture',
    defaultLabel: 'Lecture',
    context: 'This is a transcript of a lecture.',
  },
  {
    id: 'podcast',
    labelKey: 'polish.scenarios.podcast',
    defaultLabel: 'Podcast',
    context: 'This is a transcript of a podcast.',
  },
] as const satisfies readonly BuiltInPolishPreset[];

export type BuiltInPolishPresetId = typeof BUILTIN_POLISH_PRESETS[number]['id'];

export const DEFAULT_POLISH_PRESET_ID: BuiltInPolishPresetId = 'general';

export function isBuiltInPolishPresetId(value: string | null | undefined): value is BuiltInPolishPresetId {
  return BUILTIN_POLISH_PRESETS.some((preset) => preset.id === value);
}

export function getBuiltInPolishPreset(id: string | null | undefined): BuiltInPolishPreset | undefined {
  return BUILTIN_POLISH_PRESETS.find((preset) => preset.id === id);
}

export function normalizePolishCustomPresets(
  presets: PolishCustomPreset[] | null | undefined,
): PolishCustomPreset[] {
  if (!Array.isArray(presets) || presets.length === 0) {
    return [];
  }

  const seenIds = new Set<string>();
  const normalized: PolishCustomPreset[] = [];

  for (let index = 0; index < presets.length; index += 1) {
    const preset = presets[index];
    if (!preset || typeof preset !== 'object') {
      continue;
    }

    const id = typeof preset.id === 'string' && preset.id.trim()
      ? preset.id.trim()
      : createImportedPolishPresetId(`${preset.name ?? ''}-${preset.context ?? ''}-${index}`);
    if (seenIds.has(id)) {
      continue;
    }

    const context = typeof preset.context === 'string' ? preset.context.trim() : '';
    if (!context) {
      continue;
    }

    const name = typeof preset.name === 'string' && preset.name.trim()
      ? preset.name.trim()
      : buildImportedPresetName(undefined, context);

    normalized.push({
      id,
      name,
      context,
    });
    seenIds.add(id);
  }

  return normalized;
}

export function getPolishPresetLabel(
  presetId: string | null | undefined,
  customPresets: PolishCustomPreset[] | null | undefined,
  t: TFunction,
): string {
  const builtIn = getBuiltInPolishPreset(presetId);
  if (builtIn) {
    return t(builtIn.labelKey, { defaultValue: builtIn.defaultLabel });
  }

  return normalizePolishCustomPresets(customPresets).find((preset) => preset.id === presetId)?.name
    || t('polish.scenarios.general', { defaultValue: 'General' });
}

export function resolvePolishPreset(
  presetId: string | null | undefined,
  customPresets: PolishCustomPreset[] | null | undefined,
  t?: TFunction,
): ResolvedPolishPreset {
  const builtIn = getBuiltInPolishPreset(presetId);
  if (builtIn) {
    return {
      id: builtIn.id,
      name: t ? t(builtIn.labelKey, { defaultValue: builtIn.defaultLabel }) : builtIn.defaultLabel,
      context: builtIn.context,
      builtIn: true,
    };
  }

  const custom = normalizePolishCustomPresets(customPresets).find((preset) => preset.id === presetId);
  if (custom) {
    return {
      id: custom.id,
      name: custom.name,
      context: custom.context,
      builtIn: false,
    };
  }

  return {
    id: DEFAULT_POLISH_PRESET_ID,
    name: t ? t('polish.scenarios.general', { defaultValue: 'General' }) : 'General',
    context: '',
    builtIn: true,
  };
}

export function getPolishPresetOptions(
  customPresets: PolishCustomPreset[] | null | undefined,
  t: TFunction,
): Array<{ value: string; label: string }> {
  const builtInOptions = BUILTIN_POLISH_PRESETS.map((preset) => ({
    value: preset.id,
    label: t(preset.labelKey, { defaultValue: preset.defaultLabel }),
  }));
  const customOptions = normalizePolishCustomPresets(customPresets).map((preset) => ({
    value: preset.id,
    label: preset.name,
  }));

  return [...builtInOptions, ...customOptions];
}

export function coercePolishPresetId(
  presetId: string | null | undefined,
  customPresets: PolishCustomPreset[] | null | undefined,
): string {
  if (isBuiltInPolishPresetId(presetId)) {
    return presetId;
  }

  return normalizePolishCustomPresets(customPresets).some((preset) => preset.id === presetId)
    ? (presetId as string)
    : DEFAULT_POLISH_PRESET_ID;
}

export function migrateLegacyPolishSelection(
  input: LegacyPolishSelectionInput,
  existingCustomPresets: PolishCustomPreset[] | null | undefined,
  preferredName?: string,
): LegacyPolishSelectionResult {
  const customPresets = normalizePolishCustomPresets(existingCustomPresets);
  const presetId = coercePolishPresetId(input.presetId, customPresets);

  if (input.presetId && presetId === input.presetId) {
    return { presetId, customPresets };
  }

  if (isBuiltInPolishPresetId(input.scenario) && input.scenario !== 'general') {
    return {
      presetId: input.scenario,
      customPresets,
    };
  }

  const context = typeof input.context === 'string' ? input.context.trim() : '';
  if (!context) {
    return {
      presetId: DEFAULT_POLISH_PRESET_ID,
      customPresets,
    };
  }

  const ensuredPreset = ensurePolishCustomPreset(customPresets, context, preferredName);
  return {
    presetId: ensuredPreset.presetId,
    customPresets: ensuredPreset.customPresets,
  };
}

export function ensurePolishCustomPreset(
  existingCustomPresets: PolishCustomPreset[] | null | undefined,
  context: string,
  preferredName?: string,
): LegacyPolishSelectionResult {
  const normalizedContext = context.trim();
  const customPresets = normalizePolishCustomPresets(existingCustomPresets);

  const existingPreset = customPresets.find(
    (preset) => preset.context.trim().toLowerCase() === normalizedContext.toLowerCase(),
  );
  if (existingPreset) {
    return {
      presetId: existingPreset.id,
      customPresets,
    };
  }

  const presetId = createImportedPolishPresetId(normalizedContext);
  const nextPresets = [
    ...customPresets,
    {
      id: presetId,
      name: buildImportedPresetName(preferredName, normalizedContext),
      context: normalizedContext,
    },
  ];

  return {
    presetId,
    customPresets: nextPresets,
  };
}

function createImportedPolishPresetId(seed: string): string {
  return `custom-${hashString(seed)}`;
}

function buildImportedPresetName(preferredName: string | undefined, context: string): string {
  const base = preferredName?.trim() || 'Imported Preset';
  return `${base} (${hashString(context).slice(0, 6)})`;
}

function hashString(value: string): string {
  let hash = 5381;
  for (let index = 0; index < value.length; index += 1) {
    hash = ((hash << 5) + hash) ^ value.charCodeAt(index);
  }

  return Math.abs(hash >>> 0).toString(16).padStart(8, '0');
}
