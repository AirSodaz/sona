import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsAutomationTab } from '../settings/SettingsAutomationTab';
import { useAutomationStore } from '../../stores/automationStore';
import { useBatchQueueStore } from '../../stores/batchQueueStore';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useProjectStore } from '../../stores/projectStore';
import type { AutomationProfile, AutomationRule } from '../../types/automation';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string, options?: Record<string, any>) => {
            if (typeof options?.defaultValue === 'string') {
                return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_match: string, variable: string) => String(options?.[variable] ?? ''));
            }
            return key;
        },
        i18n: { language: 'en' },
    }),
    initReactI18next: { type: '3rdParty', init: () => undefined },
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

function createRule(overrides: Partial<AutomationRule> = {}): AutomationRule {
    return {
        id: 'rule-1',
        name: 'Meeting Inbox',
        kind: 'file',
        priority: 0,
        profileSource: 'tag_match',
        saveHistory: true,
        tagIds: ['project-1'],
        presetId: 'custom',
        watchDirectory: 'C:\\watch',
        recursive: true,
        enabled: true,
        actions: { autoPolish: false, autoTranslate: false, autoSummary: false },
        stageConfig: { autoPolish: false, autoTranslate: false, exportEnabled: true },
        exportConfig: { directory: 'C:\\exports', format: 'txt', mode: 'original' },
        createdAt: 1,
        updatedAt: 1,
        ...overrides,
    };
}

const profile: AutomationProfile = {
    id: 'profile-1',
    name: 'Meetings',
    translationLanguage: 'ja',
    polishPresetId: 'general',
    summaryTemplateId: 'general',
    enabledTextReplacementSetIds: [],
    enabledHotwordSetIds: [],
    enabledPolishKeywordSetIds: [],
    enabledSpeakerProfileIds: [],
    createdAt: 1,
    updatedAt: 1,
};

describe('SettingsAutomationTab', () => {
    const saveRule = vi.fn();
    const saveProfile = vi.fn();
    const applyTagRuleToExisting = vi.fn();
    const alert = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                translationLanguage: 'ja',
                polishCustomPresets: [],
                summaryCustomTemplates: [],
                textReplacementSets: [],
                hotwordSets: [],
                polishKeywordSets: [],
                speakerProfiles: [],
            },
        });
        useProjectStore.setState({
            projects: [{
                id: 'project-1',
                name: 'Team Sync',
                description: 'Meetings',
                icon: '',
                createdAt: 1,
                updatedAt: 1,
            }],
            activeProjectId: 'project-1',
        });
        useBatchQueueStore.setState({ queueItems: [] } as any);
        useDialogStore.setState({
            ...useDialogStore.getState(),
            alert: alert.mockResolvedValue(undefined),
            confirm: vi.fn().mockResolvedValue(true),
            showError: vi.fn().mockResolvedValue(undefined),
        });
        useAutomationStore.setState({
            rules: [createRule()],
            profiles: [profile],
            runtimeStates: {},
            focusTagId: null,
            saveRule: saveRule.mockResolvedValue(undefined),
            saveProfile: saveProfile.mockResolvedValue(undefined),
            deleteProfile: vi.fn().mockResolvedValue(undefined),
            deleteRule: vi.fn().mockResolvedValue(undefined),
            toggleRuleEnabled: vi.fn().mockResolvedValue(undefined),
            scanRuleNow: vi.fn().mockResolvedValue(undefined),
            retryFailed: vi.fn().mockResolvedValue(undefined),
            applyTagRuleToExisting: applyTagRuleToExisting.mockResolvedValue(2),
        });
    });

    const expandRule = () => fireEvent.click(screen.getByText('Meeting Inbox').closest('button')!);

    it('separates profile and file automation and keeps export only in file rules', () => {
        render(<SettingsAutomationTab />);

        screen.getByRole('tab', { name: 'Profiles' });
        expect(screen.queryByRole('tab', { name: 'Tag Automation' })).toBeNull();
        expect(screen.getByRole('tab', { name: 'File Automation' }).getAttribute('aria-selected')).toBe('true');
        expandRule();
        screen.getByRole('switch', { name: 'Auto-Export' });
        expect(screen.queryByRole('switch', { name: 'Auto-Polish' })).toBeNull();
        expect(screen.queryByRole('switch', { name: 'Auto-Translate' })).toBeNull();
    });

    it('creates a file rule with watcher, output Tag, profile source, and export settings', async () => {
        useAutomationStore.setState({ ...useAutomationStore.getState(), rules: [] });
        render(<SettingsAutomationTab />);

        fireEvent.click(screen.getByRole('button', { name: 'New Rule' }));
        fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), { target: { value: 'Subtitle Inbox' } });
        fireEvent.change(screen.getByPlaceholderText('Choose a folder to monitor...'), { target: { value: 'C:\\watch\\subs' } });
        fireEvent.change(screen.getByPlaceholderText('Choose where exports should be written...'), { target: { value: 'C:\\exports\\subs' } });
        fireEvent.click(screen.getByRole('checkbox', { name: 'Team Sync' }));
        fireEvent.click(screen.getByRole('switch', { name: 'Watch Subfolders' }));
        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => expect(saveRule).toHaveBeenCalledWith(expect.objectContaining({
            kind: 'file',
            name: 'Subtitle Inbox',
            tagIds: ['project-1'],
            watchDirectory: 'C:\\watch\\subs',
            recursive: true,
            profileSource: 'explicit',
            stageConfig: expect.objectContaining({ exportEnabled: true }),
            exportConfig: expect.objectContaining({ directory: 'C:\\exports\\subs' }),
        })));
    });

    it('does not expose legacy Tag automation rules', async () => {
        render(<SettingsAutomationTab />);
        expect(screen.queryByRole('tab', { name: 'Tag Automation' })).toBeNull();
        expect(saveRule).not.toHaveBeenCalled();
    });

    it('creates and duplicates reusable configuration profiles', async () => {
        render(<SettingsAutomationTab />);
        fireEvent.click(screen.getByRole('tab', { name: 'Profiles' }));
        fireEvent.click(screen.getByRole('button', { name: 'New Profile' }));
        fireEvent.change(screen.getByPlaceholderText('e.g. Customer interviews'), { target: { value: 'Interviews' } });
        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => expect(saveProfile).toHaveBeenCalledWith(expect.objectContaining({
            id: undefined,
            name: 'Interviews',
            translationLanguage: 'ja',
        })));

        fireEvent.click(screen.getByRole('button', { name: 'Duplicate' }));
        await waitFor(() => expect(saveProfile).toHaveBeenCalledWith(expect.objectContaining({
            id: undefined,
            name: 'Meetings Copy',
        })));
    });

    it('does not offer legacy Tag automation replay for existing records', async () => {
        useAutomationStore.setState({
            ...useAutomationStore.getState(),
            rules: [createRule({
                id: 'tag-rule',
                name: 'Existing meetings',
                kind: 'tag',
                tagIds: ['project-1'],
                stageConfig: { autoPolish: true, autoTranslate: false, exportEnabled: false },
                exportConfig: { directory: '', format: 'txt', mode: 'original' },
            })],
        });
        render(<SettingsAutomationTab />);
        expect(screen.queryByRole('button', { name: 'Apply to existing' })).toBeNull();
        expect(applyTagRuleToExisting).not.toHaveBeenCalled();
    });
});
