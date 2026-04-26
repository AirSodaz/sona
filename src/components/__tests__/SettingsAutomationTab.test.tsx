import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsAutomationTab } from '../settings/SettingsAutomationTab';
import { useAutomationStore } from '../../stores/automationStore';
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

        useAutomationStore.setState({
            rules: [createRule()],
            runtimeStates: {
                'rule-1': {
                    ruleId: 'rule-1',
                    status: 'watching',
                    failureCount: 2,
                    lastResult: 'error',
                    lastResultMessage: 'Translation model is required.',
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

    it('renders rule card metadata including template, recent result, and failure summary', () => {
        render(<SettingsAutomationTab />);

        expect(screen.getByText('Meeting Inbox')).toBeDefined();
        expect(screen.getByText('Team Sync')).toBeDefined();
        expect(screen.getByText('Meeting Notes')).toBeDefined();
        expect(screen.getByText('Watching')).toBeDefined();
        expect(screen.getByText('Failed')).toBeDefined();
        expect(screen.getByText('2 failures')).toBeDefined();
        expect(screen.getByText(/Watch Directory: C:\\watch/)).toBeDefined();
        expect(screen.getByText(/Output Directory: C:\\exports/)).toBeDefined();
        expect(screen.getByText('Translation model is required.')).toBeDefined();
    });

    it('renders the template controls inside the template card', () => {
        render(<SettingsAutomationTab />);

        expandRuleCard();

        expect(screen.getByRole('button', { name: 'Apply Template' })).toBeDefined();
        expect(screen.getByText('Current Template')).toBeDefined();
        expect(screen.getAllByText('Template-controlled')).toHaveLength(5);
        expect(screen.getByRole('switch', { name: 'Auto-Polish' })).toBeDefined();
        expect(screen.getByRole('switch', { name: 'Auto-Translate' })).toBeDefined();
        expect(screen.getByRole('switch', { name: 'Auto-Export' })).toBeDefined();
        expect(screen.getByRole('button', { name: 'Export Format' })).toBeDefined();
        expect(screen.getByRole('button', { name: 'Export Mode' })).toBeDefined();
        expect(screen.getByText('Templates do not change these project-level defaults.')).toBeDefined();
    });

    it('marks the rule as custom when a template-controlled field is changed and persists custom on save', async () => {
        render(<SettingsAutomationTab />);

        expandRuleCard();
        fireEvent.click(screen.getByRole('switch', { name: 'Auto-Translate' }));

        expect(screen.getAllByText('Custom').length).toBeGreaterThan(0);

        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => {
            expect(saveRuleMock).toHaveBeenCalledWith(expect.objectContaining({
                id: 'rule-1',
                presetId: 'custom',
                stageConfig: expect.objectContaining({
                    autoPolish: true,
                    autoTranslate: true,
                    exportEnabled: true,
                }),
                exportConfig: expect.objectContaining({
                    format: 'txt',
                    mode: 'original',
                }),
            }));
        });
    });

    it('keeps custom selected until apply and then restores the chosen built-in template', async () => {
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

        expect(screen.queryAllByText('Custom')).toHaveLength(1);

        expandRuleCard();
        expect(screen.queryAllByText('Custom')).toHaveLength(3);
        chooseDropdownOption('Template', 'Bilingual Subtitles');

        expect(screen.getByRole('button', { name: 'Template' }).textContent).toContain('Custom');
        expect(screen.getByText('Selected to apply: Bilingual Subtitles')).toBeDefined();
        expect(screen.getByRole('switch', { name: 'Auto-Polish' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Translation');

        fireEvent.click(screen.getByRole('button', { name: 'Apply Template' }));

        expect(screen.getAllByText('Bilingual Subtitles').length).toBeGreaterThan(0);
        expect(screen.getByRole('switch', { name: 'Auto-Polish' }).getAttribute('aria-checked')).toBe('false');
        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByRole('button', { name: 'Export Format' }).textContent).toContain('SRT');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Bilingual');

        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => {
            expect(saveRuleMock).toHaveBeenCalledWith(expect.objectContaining({
                presetId: 'bilingual_subtitles',
                stageConfig: expect.objectContaining({
                    autoPolish: false,
                    autoTranslate: true,
                    exportEnabled: true,
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

    it('changes template only after apply for a new rule and preserves non-template fields', async () => {
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

        fireEvent.click(screen.getByRole('switch', { name: 'Watch Subfolders' }));
        chooseDropdownOption('Template', 'Bilingual Subtitles');

        expect(screen.getByRole('switch', { name: 'Auto-Polish' }).getAttribute('aria-checked')).toBe('true');
        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('false');
        expect(screen.getByRole('button', { name: 'Export Format' }).textContent).toContain('TXT');
        expect(screen.getByRole('button', { name: 'Export Mode' }).textContent).toContain('Original');

        fireEvent.click(screen.getByRole('button', { name: 'Apply Template' }));

        expect(screen.getByRole('switch', { name: 'Auto-Polish' }).getAttribute('aria-checked')).toBe('false');
        expect(screen.getByRole('switch', { name: 'Auto-Translate' }).getAttribute('aria-checked')).toBe('true');
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
                presetId: 'bilingual_subtitles',
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
                }),
            }));
        });
    });
});
