import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAutomationStore } from '../../../../../stores/automationStore';
import { useDialogStore } from '../../../../../stores/dialogStore';
import { useProjectStore } from '../../../../../stores/projectStore';
import type { AutomationRule } from '../../../../../types/automation';
import { ToolAutomationPanel } from '../ToolAutomationPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string; count?: number; name?: string }) => {
      const template = options?.defaultValue ?? key;
      return template
        .replace('{{count}}', String(options?.count ?? ''))
        .replace('{{name}}', options?.name ?? '');
    },
    i18n: { language: 'en' },
  }),
  initReactI18next: { type: '3rdParty', init: () => undefined },
}));

function createRule(overrides: Partial<AutomationRule> = {}): AutomationRule {
  return {
    id: 'rule-1',
    name: 'Meeting Inbox',
    saveHistory: true,
    projectId: 'inbox',
    watchDirectory: 'C:\\watch',
    recursive: false,
    enabled: false,
    stageConfig: {
      autoPolish: false,
      autoTranslate: false,
    },
    exportConfig: {
      directory: '',
      format: 'txt',
      mode: 'original',
      prefix: '',
    },
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

describe('ToolAutomationPanel', () => {
  const saveRule = vi.fn();
  const deleteRule = vi.fn();
  const alert = vi.fn().mockResolvedValue(undefined);
  const confirm = vi.fn().mockResolvedValue(true);

  beforeEach(() => {
    saveRule.mockReset();
    saveRule.mockResolvedValue(createRule());
    deleteRule.mockReset();
    deleteRule.mockResolvedValue(undefined);
    alert.mockClear();
    confirm.mockReset();
    confirm.mockResolvedValue(true);
    useDialogStore.setState({ alert, confirm });
    useProjectStore.setState({
      projects: [{ id: 'project-1', name: 'Team', createdAt: 1, updatedAt: 1 }],
    });
    useAutomationStore.setState({
      rules: [],
      processedEntries: [],
      runtimeStates: {},
      isLoaded: true,
      error: null,
      saveRule,
      deleteRule,
      toggleRuleEnabled: vi.fn(),
      scanRuleNow: vi.fn(),
      retryFailed: vi.fn(),
      retryFailedFile: vi.fn(),
    });
  });

  it('saves a project-bound rule without requiring an export directory', async () => {
    render(<ToolAutomationPanel />);
    fireEvent.click(screen.getByRole('button', { name: '+ New Rule' }));
    fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), {
      target: { value: 'Meeting Inbox' },
    });
    fireEvent.change(screen.getByPlaceholderText('Choose a folder to monitor'), {
      target: { value: 'C:\\watch' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(saveRule).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Meeting Inbox',
        saveHistory: true,
        projectId: 'inbox',
        watchDirectory: 'C:\\watch',
        exportConfig: expect.objectContaining({ directory: '' }),
      })
    );
  });

  it('saves an export-only rule with bilingual mode', async () => {
    render(<ToolAutomationPanel />);
    fireEvent.click(screen.getByRole('button', { name: '+ New Rule' }));
    fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), {
      target: { value: 'Podcast Export' },
    });
    fireEvent.change(screen.getByPlaceholderText('Choose a folder to monitor'), {
      target: { value: 'C:\\raw' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Export Only (No History)' }));
    fireEvent.change(screen.getByPlaceholderText('Choose an export directory'), {
      target: { value: 'C:\\out' },
    });
    fireEvent.click(screen.getByRole('switch', { name: 'Auto Translate' }));
    fireEvent.click(screen.getByRole('button', { name: 'Export Mode' }));
    fireEvent.click(screen.getByRole('option', { name: 'bilingual' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(saveRule).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Podcast Export',
        saveHistory: false,
        projectId: '',
        exportConfig: expect.objectContaining({
          directory: 'C:\\out',
          format: 'txt',
          mode: 'bilingual',
        }),
      })
    );
  });

  it('shows_loading_instead_of_the_empty_state_before_rules_load', () => {
    useAutomationStore.setState({ isLoaded: false, rules: [], error: null });

    render(<ToolAutomationPanel />);

    expect(screen.getByRole('button', { name: '+ New Rule' }).hasAttribute('disabled')).toBe(true);
    expect(screen.queryByText('No automation rules yet.')).toBeNull();
    expect(screen.getByText('Loading...')).toBeDefined();
  });

  it('keeps an unresolvable target project when the rule is saved untouched', async () => {
    useProjectStore.setState({ projects: [] });
    useAutomationStore.setState({
      rules: [createRule({ projectId: 'project-9' })],
      isLoaded: true,
    });

    render(<ToolAutomationPanel />);
    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(saveRule).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'rule-1', projectId: 'project-9' })
      )
    );
  });

  it('shows the runtime failure detail on the rule card', () => {
    useAutomationStore.setState({
      rules: [createRule({ enabled: true })],
      runtimeStates: {
        'rule-1': {
          ruleId: 'rule-1',
          status: 'error',
          lastResultMessage: 'Watch directory is unavailable',
          failureCount: 1,
        },
      },
      processedEntries: [],
    });

    render(<ToolAutomationPanel />);

    expect(screen.getByText('Watch directory is unavailable')).toBeDefined();
  });

  it('shows the load failure with a retry action instead of the empty state', () => {
    useAutomationStore.setState({ isLoaded: false, rules: [], error: 'Database is locked' });

    render(<ToolAutomationPanel />);

    expect(screen.getByText('Database is locked')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeDefined();
    expect(screen.queryByText('No automation rules yet.')).toBeNull();
  });

  it('retries a failed file from the rule card', () => {
    const retryFailedFile = vi.fn();
    useAutomationStore.setState({
      rules: [createRule({ enabled: true })],
      processedEntries: [
        {
          ruleId: 'rule-1',
          attempt: 1,
          filePath: 'C:\\raw\\bad.wav',
          sourceFingerprint: 'fp',
          size: 1,
          mtimeMs: 1,
          status: 'error',
          processedAt: 1,
          errorMessage: 'ASR missing',
        },
      ],
      retryFailedFile,
    });

    render(<ToolAutomationPanel />);
    expect(screen.getByText('bad.wav')).toBeDefined();
    expect(screen.getByText('ASR missing')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(retryFailedFile).toHaveBeenCalledWith('rule-1', 'C:\\raw\\bad.wav');
  });
  it('blocks save when the rule name or watch directory is missing', async () => {
    render(<ToolAutomationPanel />);
    fireEvent.click(screen.getByRole('button', { name: '+ New Rule' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(alert).toHaveBeenCalledTimes(1));
    expect(saveRule).not.toHaveBeenCalled();
  });

  it('requires an output directory for export-only rules', async () => {
    render(<ToolAutomationPanel />);
    fireEvent.click(screen.getByRole('button', { name: '+ New Rule' }));
    fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), {
      target: { value: 'Export Only' },
    });
    fireEvent.change(screen.getByPlaceholderText('Choose a folder to monitor'), {
      target: { value: 'C:\\watch' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Export Only (No History)' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(alert).toHaveBeenCalledTimes(1));
    expect(saveRule).not.toHaveBeenCalled();
  });

  it('asks for confirmation before deleting a rule', async () => {
    useAutomationStore.setState({ rules: [createRule()] });
    render(<ToolAutomationPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(deleteRule).toHaveBeenCalledWith('rule-1'));

    deleteRule.mockClear();
    confirm.mockResolvedValue(false);
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(confirm).toHaveBeenCalled());
    expect(deleteRule).not.toHaveBeenCalled();
  });

  it('edits an existing rule keeping its id', async () => {
    useAutomationStore.setState({ rules: [createRule()] });
    render(<ToolAutomationPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    fireEvent.change(screen.getByPlaceholderText('e.g. Weekly Meeting Inbox'), {
      target: { value: 'Renamed Rule' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(saveRule).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'rule-1', name: 'Renamed Rule' })
      )
    );
  });
});
