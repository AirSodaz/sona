import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsAutomationTab } from '../settings/SettingsAutomationTab';
import { useAutomationStore } from '../../stores/automationStore';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useProjectStore } from '../../stores/projectStore';
import type { AutomationRule } from '../../types/automation';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string, options?: Record<string, any>) => {
            if (typeof options?.defaultValue === 'string') {
                return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_match: string, variable: string) => String(options?.[variable] ?? ''));
            }
            return key;
        },
    }),
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
}));

function createRule(overrides: Partial<AutomationRule> = {}): AutomationRule {
    return {
        id: 'rule-1',
        name: 'Meeting Inbox',
        projectId: 'project-1',
        presetId: 'meeting_notes',
        watchDirectory: 'C:\\watch',
        recursive: true,
        enabled: true,
        stageConfig: {
            autoPolish: true,
            autoTranslate: false,
            exportEnabled: true,
        },
        exportConfig: {
            directory: 'C:\\exports',
            format: 'txt',
            mode: 'original',
        },
        createdAt: 1,
        updatedAt: 1,
        ...overrides,
    };
}

describe('SettingsAutomationTab', () => {
    const saveRuleMock = vi.fn();
    const deleteRuleMock = vi.fn();
    const toggleRuleEnabledMock = vi.fn();
    const scanRuleNowMock = vi.fn();
    const retryFailedMock = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();

        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                translationLanguage: 'ja',
                polishCustomPresets: [],
            },
        });

        useProjectStore.setState({
            projects: [
                {
                    id: 'project-1',
                    name: 'Team Sync',
                    description: 'Meetings',
                    icon: '🧪',
                    createdAt: 1,
                    updatedAt: 1,
                    defaults: {
                        summaryTemplateId: 'general',
                        translationLanguage: 'ja',
                        polishPresetId: 'general',
                        exportFileNamePrefix: 'TEAM',
                        enabledTextReplacementSetIds: [],
                        enabledHotwordSetIds: [],
                        enabledPolishKeywordSetIds: [],
                        enabledSpeakerProfileIds: [],
                    },
                },
            ],
            activeProjectId: 'project-1',
            isLoading: false,
            error: null,
        });

        useDialogStore.setState({
            ...useDialogStore.getState(),
            alert: vi.fn().mockResolvedValue(undefined),
            confirm: vi.fn().mockResolvedValue(true),
        });

        useBatchQueueStore.setState({
            queueItems: [
                {
                    id: 'queue-pending',
                    filename: 'pending.wav',
                    filePath: 'C:\\watch\\pending.wav',
                    status: 'pending',
                    progress: 0,
                    segments: [],
                    projectId: 'project-1',
                    origin: 'automation',
                    automationRuleId: 'rule-1',
                },
                {
                    id: 'queue-processing',
                    filename: 'processing.wav',
                    filePath: 'C:\\watch\\processing.wav',
                    status: 'processing',
                    progress: 48,
                    segments: [],
                    projectId: 'project-1',
                    origin: 'automation',
                    automationRuleId: 'rule-1',
                },
            ] as any,
        });

        useAutomationStore.setState({
            rules: [createRule()],
            runtimeStates: {
                'rule-1': {
                    ruleId: 'rule-1',
                    status: 'watching',
                    failureCount: 2,
                    lastResult: 'error',
                    lastResultMessage: 'Translation model is required.',
                    lastQueuedAt: 100,
                    lastBlockedAt: 200,
                    lastBlockedReason: 'already_pending',
                    lastBlockedFilePath: 'C:\\watch\\duplicate.wav',
                },
            },
            saveRule: saveRuleMock.mockResolvedValue(undefined),
            deleteRule: deleteRuleMock.mockResolvedValue(undefined),
            toggleRuleEnabled: toggleRuleEnabledMock.mockResolvedValue(undefined),
            scanRuleNow: scanRuleNowMock.mockResolvedValue(undefined),
            retryFailed: retryFailedMock.mockResolvedValue(undefined),
        });
    });

    const expandRuleCard = () => {
        const summaryButton = screen.getByText('Meeting Inbox').closest('button');
        if (!summaryButton) {
            throw new Error('Rule summary button not found');
        }
        fireEvent.click(summaryButton);
    };

    const chooseDropdownOption = (triggerName: string, optionName: string) => {
        fireEvent.click(screen.getByRole('button', { name: triggerName }));
        fireEvent.click(screen.getByRole('option', { name: optionName }));
    };

    it('renders rule card metadata including recent result, queue summary, and blocked hint', () => {
        render(<SettingsAutomationTab />);

        expect(screen.getByText('Meeting Inbox')).toBeDefined();
        expect(screen.getByText('Team Sync')).toBeDefined();
        expect(screen.getByText('Watching')).toBeDefined();
        expect(screen.getByText('Failed')).toBeDefined();
        expect(screen.getByText('2 failures')).toBeDefined();
        expect(screen.getByText('1 pending')).toBeDefined();
        expect(screen.getByText('1 processing')).toBeDefined();
        expect(screen.getByText(/Watch Directory: C:\\watch/)).toBeDefined();
        expect(screen.getByText(/Output Directory: C:\\exports/)).toBeDefined();
        expect(screen.getByText('Translation model is required.')).toBeDefined();
        expect(screen.getByText('Skipped duplicate.wav: already queued')).toBeDefined();
    });

    it('renders the direct stage controls inside the expanded rule card', () => {
        render(<SettingsAutomationTab />);

        expandRuleCard();

        expect(screen.getByRole('switch', { name: 'Auto-Polish' })).toBeDefined();
        expect(screen.getByRole('switch', { name: 'Auto-Translate' })).toBeDefined();
        expect(screen.getByRole('switch', { name: 'Auto-Export' })).toBeDefined();
        expect(screen.getByRole('button', { name: 'Polish Preset' })).toBeDefined();
        expect(screen.getByRole('button', { name: 'Export Format' })).toBeDefined();
        expect(screen.getByRole('button', { name: 'Export Mode' })).toBeDefined();
        expect(screen.getByRole('button', { name: 'Target Project' })).toBeDefined();
        expect(screen.getByRole('switch', { name: 'Watch Subfolders' })).toBeDefined();
        expect(screen.queryByRole('button', { name: 'Target Language' })).toBeNull();
        expect(screen.queryByRole('button', { name: 'Apply Template' })).toBeNull();
    });

    it('marks the rule as custom when a stage-controlled field is changed and persists custom on save', async () => {
        render(<SettingsAutomationTab />);

        expandRuleCard();
        fireEvent.click(screen.getByRole('switch', { name: 'Auto-Translate' }));

        expect(screen.getByRole('button', { name: 'Target Language' })).toBeDefined();

        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => {
            expect(saveRuleMock).toHaveBeenCalledWith(expect.objectContaining({
                id: 'rule-1',
                presetId: 'custom',
                stageConfig: expect.objectContaining({
                    autoPolish: true,
                    autoTranslate: true,
                    exportEnabled: true,
                    translationLanguage: 'en',
                }),
                exportConfig: expect.objectContaining({
                    format: 'txt',
                    mode: 'original',
                }),
            }));
        });
    });

    it('loads an existing custom rule into the editor and preserves updated custom selections on save', async () => {
        useAutomationStore.setState({
            ...useAutomationStore.getState(),
            rules: [
                createRule({
                    presetId: 'custom',
                    stageConfig: {
                        autoPolish: true,
                        autoTranslate: true,
                        exportEnabled: true,
                    },
                    exportConfig: {
                        directory: 'C:\\exports',
                        format: 'txt',
                        mode: 'translation',
                    },
                }),
            ],
        });

        render(<SettingsAutomationTab />);

        expandRuleCard();
        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Translation');

        chooseDropdownOption('Export Mode', 'Bilingual');
        chooseDropdownOption('Export Format', 'SRT');

        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => {
            expect(saveRuleMock).toHaveBeenCalledWith(expect.objectContaining({
                presetId: 'custom',
                stageConfig: expect.objectContaining({
                    autoPolish: true,
                    autoTranslate: true,
                    exportEnabled: true,
                    translationLanguage: 'en',
                }),
                exportConfig: expect.objectContaining({
                    format: 'srt',
                    mode: 'bilingual',
                }),
            }));
        });
    });

    it('forces export mode back to original when auto-translate is turned off and restores choices when re-enabled', () => {
        useAutomationStore.setState({
            ...useAutomationStore.getState(),
            rules: [
                createRule({
                    presetId: 'custom',
                    stageConfig: {
                        autoPolish: true,
                        autoTranslate: true,
                        exportEnabled: true,
                    },
                    exportConfig: {
                        directory: 'C:\\exports',
                        format: 'txt',
                        mode: 'translation',
                    },
                }),
            ],
        });

        render(<SettingsAutomationTab />);

        expandRuleCard();
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Translation');

        fireEvent.click(screen.getByRole('switch', { name: 'Auto-Translate' }));

        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('false');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Original');

        fireEvent.click(screen.getByRole('button', { name: 'Export Mode' }));
        expect(screen.getByRole('option', { name: 'Original' })).toBeDefined();
        expect(screen.queryByRole('option', { name: 'Translation' })).toBeNull();
        expect(screen.queryByRole('option', { name: 'Bilingual' })).toBeNull();
        fireEvent.click(screen.getByRole('option', { name: 'Original' }));

        fireEvent.click(screen.getByRole('switch', { name: 'Auto-Translate' }));
        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Original');

        fireEvent.click(screen.getByRole('button', { name: 'Export Mode' }));
        expect(screen.getByRole('option', { name: 'Original' })).toBeDefined();
        expect(screen.getByRole('option', { name: 'Translation' })).toBeDefined();
        expect(screen.getByRole('option', { name: 'Bilingual' })).toBeDefined();
    });

    it('normalizes invalid saved draft state when auto-translate is off but export mode was persisted as translation', async () => {
        useAutomationStore.setState({
            ...useAutomationStore.getState(),
            rules: [
                createRule({
                    presetId: 'custom',
                    stageConfig: {
                        autoPolish: true,
                        autoTranslate: false,
                        exportEnabled: true,
                    },
                    exportConfig: {
                        directory: 'C:\\exports',
                        format: 'txt',
                        mode: 'translation',
                    },
                }),
            ],
        });

        render(<SettingsAutomationTab />);

        expandRuleCard();
        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('false');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Original');

        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => {
            expect(saveRuleMock).toHaveBeenCalledWith(expect.objectContaining({
                presetId: 'custom',
                stageConfig: expect.objectContaining({
                    autoTranslate: false,
                }),
                exportConfig: expect.objectContaining({
                    mode: 'original',
                }),
            }));
        });
    });

    it('creates a new rule through the direct stage controls and preserves non-stage fields', async () => {
        render(<SettingsAutomationTab />);

        fireEvent.click(screen.getByRole('button', { name: 'New Rule' }));

        fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), {
            target: { value: 'Subtitle Inbox' },
        });
        fireEvent.change(screen.getByPlaceholderText('Choose a folder to monitor...'), {
            target: { value: 'C:\\watch\\subs' },
        });
        fireEvent.change(screen.getByPlaceholderText('Choose where exports should be written...'), {
            target: { value: 'C:\\exports\\subs' },
        });

        chooseDropdownOption('Target Project', 'Team Sync');
        fireEvent.click(screen.getByRole('switch', { name: 'Auto-Translate' }));
        fireEvent.click(screen.getByRole('switch', { name: 'Auto-Export' }));
        fireEvent.click(screen.getByRole('switch', { name: 'Watch Subfolders' }));
        chooseDropdownOption('Export Format', 'SRT');
        chooseDropdownOption('Export Mode', 'Bilingual');

        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByRole('switch', { name: 'Auto-Export' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByRole('button', { name: 'Export Format' }).textContent).toContain('SRT');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Bilingual');
        expect(screen.getByRole('switch', { name: 'Watch Subfolders' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByDisplayValue('C:\\watch\\subs')).toBeDefined();
        expect(screen.getByDisplayValue('C:\\exports\\subs')).toBeDefined();

        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => {
            expect(saveRuleMock).toHaveBeenCalledWith(expect.objectContaining({
                name: 'Subtitle Inbox',
                projectId: 'project-1',
                presetId: 'custom',
                watchDirectory: 'C:\\watch\\subs',
                recursive: true,
                exportConfig: expect.objectContaining({
                    directory: 'C:\\exports\\subs',
                    format: 'srt',
                    mode: 'bilingual',
                }),
                stageConfig: expect.objectContaining({
                    autoPolish: false,
                    autoTranslate: true,
                    exportEnabled: true,
                    translationLanguage: 'en',
                }),
            }));
        });
    });
});
