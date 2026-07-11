import type {
  AutomationPresetDefinition,
  AutomationPresetId,
  AutomationRule,
  BuiltInAutomationPresetId,
} from '../types/automation';

export const AUTOMATION_PRESETS = [
  {
    id: 'meeting_notes',
    labelKey: 'automation.presets.meeting_notes',
    defaultLabel: 'Meeting Notes',
    descriptionKey: 'automation.presets.meeting_notes_description',
    defaultDescription: 'Polish the transcript and export clean meeting notes as plain text.',
    stageConfig: {
      autoPolish: true,
      autoTranslate: false,
      exportEnabled: true,
    },
    exportConfig: {
      format: 'txt',
      mode: 'original',
    },
  },
  {
    id: 'lecture_notes',
    labelKey: 'automation.presets.lecture_notes',
    defaultLabel: 'Lecture Notes',
    descriptionKey: 'automation.presets.lecture_notes_description',
    defaultDescription: 'Polish the transcript and export lecture notes as plain text.',
    stageConfig: {
      autoPolish: true,
      autoTranslate: false,
      exportEnabled: true,
    },
    exportConfig: {
      format: 'txt',
      mode: 'original',
    },
  },
  {
    id: 'bilingual_subtitles',
    labelKey: 'automation.presets.bilingual_subtitles',
    defaultLabel: 'Bilingual Subtitles',
    descriptionKey: 'automation.presets.bilingual_subtitles_description',
    defaultDescription: 'Translate the transcript and export bilingual subtitles.',
    stageConfig: {
      autoPolish: false,
      autoTranslate: true,
      exportEnabled: true,
    },
    exportConfig: {
      format: 'srt',
      mode: 'bilingual',
    },
  },
] as const satisfies readonly AutomationPresetDefinition[];

export const DEFAULT_AUTOMATION_PRESET_ID: BuiltInAutomationPresetId = 'meeting_notes';

const CUSTOM_AUTOMATION_PRESET: AutomationPresetDefinition = {
  id: 'custom',
  labelKey: 'automation.presets.custom',
  defaultLabel: 'Custom',
  descriptionKey: 'automation.presets.custom_description',
  defaultDescription: 'This rule was manually adjusted after applying a built-in template.',
  stageConfig: {
    autoPolish: false,
    autoTranslate: false,
    exportEnabled: false,
  },
  exportConfig: {
    format: 'txt',
    mode: 'original',
  },
};

export function isBuiltInAutomationPresetId(
  presetId: AutomationPresetId | string | null | undefined,
): presetId is BuiltInAutomationPresetId {
  return AUTOMATION_PRESETS.some((preset) => preset.id === presetId);
}

export function getAutomationPresetDefinition(presetId: AutomationPresetId | string | null | undefined): AutomationPresetDefinition {
  if (presetId === 'custom') {
    return CUSTOM_AUTOMATION_PRESET;
  }

  return AUTOMATION_PRESETS.find((preset) => preset.id === presetId)
    ?? AUTOMATION_PRESETS[0];
}

export function applyAutomationPreset(
  presetId: BuiltInAutomationPresetId,
  rule: Pick<AutomationRule, 'stageConfig' | 'exportConfig'>,
): Pick<AutomationRule, 'stageConfig' | 'exportConfig'> {
  const preset = getAutomationPresetDefinition(presetId);
  return {
    stageConfig: {
      ...preset.stageConfig,
    },
    exportConfig: {
      ...rule.exportConfig,
      format: preset.exportConfig.format,
      mode: preset.exportConfig.mode,
    },
  };
}

export function matchesAutomationPreset(
  presetId: BuiltInAutomationPresetId,
  rule: Pick<AutomationRule, 'stageConfig' | 'exportConfig'>,
): boolean {
  const preset = getAutomationPresetDefinition(presetId);

  return (
    rule.stageConfig.autoPolish === preset.stageConfig.autoPolish
    && rule.stageConfig.autoTranslate === preset.stageConfig.autoTranslate
    && rule.stageConfig.exportEnabled === preset.stageConfig.exportEnabled
    && rule.exportConfig.format === preset.exportConfig.format
    && rule.exportConfig.mode === preset.exportConfig.mode
  );
}

export function findMatchingAutomationPreset(
  rule: Pick<AutomationRule, 'stageConfig' | 'exportConfig'>,
): BuiltInAutomationPresetId | null {
  return AUTOMATION_PRESETS.find((preset) => matchesAutomationPreset(preset.id, rule))?.id ?? null;
}
