import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAutomationStore } from '../../../../../stores/automationStore';
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

  beforeEach(() => {
    saveRule.mockReset();
    saveRule.mockResolvedValue(createRule());
    useProjectStore.setState({
      projects: [{ id: 'project-1', name: 'Team', createdAt: 1, updatedAt: 1 }],
    });
    useAutomationStore.setState({
      rules: [],
      processedEntries: [],
      runtimeStates: {},
      saveRule,
      deleteRule: vi.fn(),
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
});
