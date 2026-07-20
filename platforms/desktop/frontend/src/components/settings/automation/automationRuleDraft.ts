import type { AutomationRule } from '../../../types/automation';
import type { ExportMode } from '../../../utils/exportFormats';

export { LANGUAGE_OPTIONS } from '../../../constants/languages';
export const NEW_RULE_KEY = '__new__';

export interface AutomationRuleDraft {
    id?: string;
    name: string;
    kind: NonNullable<AutomationRule['kind']>;
    priority: number;
    profileId?: string;
    profileSource?: string;
    actions: NonNullable<AutomationRule['actions']>;
    migrationNotice?: string;
    saveHistory: boolean;
    tagIds: string[];
    presetId: AutomationRule['presetId'];
    watchDirectory: string;
    recursive: boolean;
    enabled: boolean;
    stageConfig: AutomationRule['stageConfig'];
    exportConfig: AutomationRule['exportConfig'];
}

export type AutomationDraftUpdate = (draft: AutomationRuleDraft) => AutomationRuleDraft;

type DirectDraftField =
    | 'enabled'
    | 'name'
    | 'kind'
    | 'priority'
    | 'profileId'
    | 'profileSource'
    | 'saveHistory'
    | 'tagIds'
    | 'recursive'
    | 'watchDirectory';

export function normalizeExportMode(autoTranslate: boolean, mode: ExportMode): ExportMode {
    if (!autoTranslate && (mode === 'translation' || mode === 'bilingual')) {
        return 'original';
    }

    return mode;
}

export function normalizeAutomationRuleDraft(draft: AutomationRuleDraft): AutomationRuleDraft {
    const normalizedMode = normalizeExportMode(draft.stageConfig.autoTranslate, draft.exportConfig.mode);

    const stageConfig = {
        polishPresetId: 'general',
        translationLanguage: 'en',
        ...draft.stageConfig,
    };

    const exportConfig = {
        prefix: '',
        ...draft.exportConfig,
        mode: normalizedMode,
    };

    if (
        normalizedMode === draft.exportConfig.mode
        && stageConfig.polishPresetId === draft.stageConfig.polishPresetId
        && stageConfig.translationLanguage === draft.stageConfig.translationLanguage
        && exportConfig.prefix === draft.exportConfig.prefix
    ) {
        return draft;
    }

    return {
        ...draft,
        stageConfig,
        exportConfig,
    };
}

export function createRuleDraft(projectId = 'inbox', kind: NonNullable<AutomationRule['kind']> = 'file'): AutomationRuleDraft {
    return normalizeAutomationRuleDraft({
        name: '',
        kind,
        priority: 0,
        profileSource: kind === 'file' ? 'explicit' : 'tag_match',
        actions: {
            autoPolish: false,
            autoTranslate: false,
            autoSummary: false,
        },
        saveHistory: projectId !== 'none',
        tagIds: projectId && projectId !== 'none' && projectId !== 'inbox' ? [projectId] : [],
        presetId: 'custom',
        watchDirectory: '',
        recursive: false,
        enabled: false,
        stageConfig: {
            autoPolish: false,
            polishPresetId: 'general',
            autoTranslate: false,
            translationLanguage: 'en',
            exportEnabled: kind === 'file',
        },
        exportConfig: {
            directory: '',
            format: 'txt',
            mode: 'original',
            prefix: '',
        },
    });
}

export function createDraftFromRule(rule: AutomationRule): AutomationRuleDraft {
    return normalizeAutomationRuleDraft({
        id: rule.id,
        name: rule.name,
        kind: rule.kind ?? 'file',
        priority: rule.priority ?? 0,
        profileId: rule.profileId,
        profileSource: rule.profileSource ?? ((rule.kind ?? 'file') === 'file' ? 'explicit' : 'tag_match'),
        actions: rule.actions ?? {
            autoPolish: rule.stageConfig.autoPolish,
            autoTranslate: rule.stageConfig.autoTranslate,
            autoSummary: false,
        },
        migrationNotice: rule.migrationNotice,
        saveHistory: rule.saveHistory ?? rule.projectId !== 'none',
        tagIds: rule.tagIds ?? (
            rule.projectId && rule.projectId !== 'none' && rule.projectId !== 'inbox'
                ? [rule.projectId]
                : []
        ),
        presetId: rule.presetId,
        watchDirectory: rule.watchDirectory,
        recursive: rule.recursive,
        enabled: rule.enabled,
        stageConfig: {
            polishPresetId: 'general',
            translationLanguage: 'en',
            ...rule.stageConfig,
        },
        exportConfig: {
            prefix: '',
            ...rule.exportConfig,
        },
    });
}

export function setDraftField<K extends DirectDraftField>(
    field: K,
    value: AutomationRuleDraft[K],
): AutomationDraftUpdate {
    return (draft) => ({
        ...draft,
        [field]: value,
    });
}

export function setStageConfigField<K extends keyof AutomationRuleDraft['stageConfig']>(
    field: K,
    value: AutomationRuleDraft['stageConfig'][K],
): AutomationDraftUpdate {
    return (draft) => ({
        ...draft,
        presetId: 'custom',
        stageConfig: {
            ...draft.stageConfig,
            [field]: value,
        },
    });
}

export function setActionField<K extends keyof AutomationRuleDraft['actions']>(
    field: K,
    value: AutomationRuleDraft['actions'][K],
): AutomationDraftUpdate {
    return (draft) => ({
        ...draft,
        actions: {
            ...draft.actions,
            [field]: value,
        },
        stageConfig: {
            ...draft.stageConfig,
            ...(field === 'autoPolish' ? { autoPolish: value } : {}),
            ...(field === 'autoTranslate' ? { autoTranslate: value } : {}),
        },
    });
}

export function setExportConfigField<K extends keyof AutomationRuleDraft['exportConfig']>(
    field: K,
    value: AutomationRuleDraft['exportConfig'][K],
    markCustom = true,
): AutomationDraftUpdate {
    return (draft) => ({
        ...draft,
        presetId: markCustom ? 'custom' : draft.presetId,
        exportConfig: {
            ...draft.exportConfig,
            [field]: value,
        },
    });
}
