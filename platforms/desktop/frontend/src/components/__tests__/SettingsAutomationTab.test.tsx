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
        t: (key: string, options?: Record<string, unknown>) => {
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
        projectId: 'project-1',
        watchDirectory: 'C:\\watch',
        recursive: true,
        enabled: true,
        saveHistory: true,
        createdAt: 1,
        updatedAt: 1,
        ...overrides,
    };
}

describe('SettingsAutomationTab', () => {
    const saveRule = vi.fn();
    const deleteRule = vi.fn();
    const toggleRuleEnabled = vi.fn();
    const alert = vi.fn();
    const confirm = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        useConfigStore.setState({
            config: {
                ...useConfigStore.getState().config,
                translationLanguage: 'ja',
            },
        });
        useProjectStore.setState({
            projects: [{
                id: 'project-1',
                name: 'Team Sync',
                description: 'Meetings',
                icon: '',
                sortOrder: 0,
                createdAt: 1,
                updatedAt: 1,
                pipeline: {
                    enabled: true,
                    autoPolish: true,
                    polishPresetId: 'meeting',
                    autoTranslate: false,
                    autoSummary: true,
                    summaryTemplateId: 'general',
                    autoExport: false,
                },
            }],
            activeProjectId: 'project-1',
        });
        useBatchQueueStore.setState({ queueItems: [] });
        useDialogStore.setState({
            ...useDialogStore.getState(),
            alert: alert.mockResolvedValue(undefined),
            confirm: confirm.mockResolvedValue(true),
            showError: vi.fn().mockResolvedValue(undefined),
        });
        useAutomationStore.setState({
            rules: [createRule()],
            profiles: [],
            runtimeStates: {},
            focusTagId: null,
            saveRule: saveRule.mockResolvedValue(undefined),
            deleteRule: deleteRule.mockResolvedValue(undefined),
            toggleRuleEnabled: toggleRuleEnabled.mockResolvedValue(undefined),
            scanRuleNow: vi.fn().mockResolvedValue(undefined),
            retryFailed: vi.fn().mockResolvedValue(undefined),
        });
    });

    it('renders folder automation watchers and project attribution label', () => {
        render(<SettingsAutomationTab />);

        expect(screen.getByText('Automation')).toBeDefined();
        expect(screen.getByText('Meeting Inbox')).toBeDefined();
        expect(screen.getByText('Team Sync')).toBeDefined();
        expect(screen.getByTitle('C:\\watch')).toBeDefined();
    });

    it('creates a new folder monitoring rule targeting a project', async () => {
        useAutomationStore.setState({ ...useAutomationStore.getState(), rules: [] });
        render(<SettingsAutomationTab />);

        fireEvent.click(screen.getByRole('button', { name: 'New Rule' }));
        fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), { target: { value: 'Customer Interviews' } });
        fireEvent.change(screen.getByPlaceholderText('Choose a folder to monitor...'), { target: { value: 'C:\\watch\\interviews' } });
        fireEvent.click(screen.getByRole('switch', { name: 'Watch Subfolders' }));
        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => expect(saveRule).toHaveBeenCalledWith(expect.objectContaining({
            name: 'Customer Interviews',
            watchDirectory: 'C:\\watch\\interviews',
            recursive: true,
            projectId: 'inbox',
        })));
    });
    it('hides target project when save to history is disabled', async () => {
        useAutomationStore.setState({ ...useAutomationStore.getState(), rules: [] });
        render(<SettingsAutomationTab />);

        fireEvent.click(screen.getByRole('button', { name: 'New Rule' }));

        const saveHistorySwitch = screen.getByRole('switch', { name: 'Save to History' });
        expect(saveHistorySwitch).toBeDefined();
        expect(screen.getByRole('button', { name: 'Target Project' })).toBeDefined();

        // Toggle Save to History off
        fireEvent.click(saveHistorySwitch);

        expect(screen.queryByRole('button', { name: 'Target Project' })).toBeNull();

        fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), { target: { value: 'Export Only Watcher' } });
        fireEvent.change(screen.getByPlaceholderText('Choose a folder to monitor...'), { target: { value: 'C:\\watch\\export_only' } });

        // First try to save without export directory -> should alert
        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));
        await waitFor(() => {
            expect(alert).toHaveBeenCalledWith(
                'Please specify an output directory when Save to History is disabled.',
                expect.any(Object),
            );
            expect(saveRule).not.toHaveBeenCalled();
        });

        // Fill export directory and save
        // Verify Export Format dropdown exists and can be selected
        const exportFormatDropdown = screen.getByRole('button', { name: 'Export Format' });
        expect(exportFormatDropdown).toBeDefined();
        fireEvent.click(exportFormatDropdown);
        fireEvent.click(screen.getByRole('option', { name: 'SRT' }));

        const browseButtons = screen.getAllByRole('button', { name: 'Browse' });
        expect(browseButtons).toHaveLength(2);
        fireEvent.change(screen.getByPlaceholderText('Choose export directory...'), { target: { value: 'C:\\exports' } });
        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => expect(saveRule).toHaveBeenCalledWith(expect.objectContaining({
            name: 'Export Only Watcher',
            watchDirectory: 'C:\\watch\\export_only',
            saveHistory: false,
            exportConfig: expect.objectContaining({
                directory: 'C:\\exports',
                format: 'srt',
            }),
        })));
    });

    it('validates rule name and directory before saving', async () => {
        useAutomationStore.setState({ ...useAutomationStore.getState(), rules: [] });
        render(<SettingsAutomationTab />);

        fireEvent.click(screen.getByRole('button', { name: 'New Rule' }));
        fireEvent.click(screen.getByRole('button', { name: 'common.save' }));

        await waitFor(() => {
            expect(alert).toHaveBeenCalledWith(
                'Complete the rule name and watch directory before saving.',
                expect.any(Object),
            );
            expect(saveRule).not.toHaveBeenCalled();
        });
    });

    it('confirms and deletes an existing rule', async () => {
        render(<SettingsAutomationTab />);

        fireEvent.click(screen.getByRole('button', { name: 'common.delete' }));

        await waitFor(() => {
            expect(confirm).toHaveBeenCalledWith(
                'Delete automation rule "Meeting Inbox"?',
                expect.any(Object),
            );
            expect(deleteRule).toHaveBeenCalledWith('rule-1');
        });
    });
});
