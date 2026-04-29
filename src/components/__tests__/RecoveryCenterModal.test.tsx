import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { RecoveryCenterModal } from '../RecoveryCenterModal';

const mockResumeAll = vi.fn();
const mockDiscardAll = vi.fn();
const mockDiscardItem = vi.fn();

const recoveryState = {
  items: [
    {
      id: 'recovery-batch-1',
      filename: 'meeting.wav',
      filePath: 'C:\\watch\\meeting.wav',
      source: 'batch_import' as const,
      resolution: 'pending' as const,
      progress: 25,
      segments: [],
      projectId: 'project-1',
      historyId: 'history-1',
      lastKnownStage: 'transcribing' as const,
      updatedAt: 100,
      hasSourceFile: true,
      canResume: true,
    },
    {
      id: 'recovery-automation-1',
      filename: 'automation.wav',
      filePath: 'C:\\watch\\automation.wav',
      source: 'automation' as const,
      resolution: 'pending' as const,
      progress: 55,
      segments: [],
      projectId: 'project-1',
      lastKnownStage: 'translating' as const,
      updatedAt: 200,
      hasSourceFile: true,
      canResume: true,
      automationRuleId: 'rule-1',
      automationRuleName: 'Inbox Rule',
      sourceFingerprint: 'fp-1',
    },
  ],
  updatedAt: 300,
  isLoaded: true,
  isBusy: false,
  error: null,
  resumeAll: (...args: unknown[]) => mockResumeAll(...args),
  discardAll: (...args: unknown[]) => mockDiscardAll(...args),
  discardItem: (...args: unknown[]) => mockDiscardItem(...args),
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'recovery.source.batch_import') return 'Batch Import';
      if (key === 'recovery.source.automation') return 'Automation';
      if (key === 'recovery.title') return 'Interrupted Batch Recovery';
      if (key === 'recovery.description') return 'Recovery description';
      if (key === 'recovery.badge') return 'Recovery Center';
      if (key === 'recovery.actions.resume_all') return 'Resume All';
      if (key === 'recovery.actions.discard_all') return 'Discard All';
      if (key === 'recovery.actions.discard') return 'Discard';
      if (key === 'recovery.overview.batch_description') return 'Batch overview';
      if (key === 'recovery.overview.automation_description') return 'Automation overview';
      if (key === 'recovery.overview.pending_count') return `${options?.count} file`;
      if (key === 'recovery.overview.draft_count') return `${options?.count} draft`;
      if (key === 'recovery.section.batch_description') return 'Batch section';
      if (key === 'recovery.section.automation_description') return 'Automation section';
      if (key === 'recovery.item.batch_description') return `Target project: ${options?.projectName}`;
      if (key === 'recovery.item.automation_description') return `Automation rule: ${options?.ruleName} · Target project: ${options?.projectName}`;
      if (key === 'recovery.labels.no_project') return 'No project';
      if (key === 'recovery.labels.partial_draft') return 'Partial draft available';
      if (key === 'recovery.labels.last_recovered') return 'Recovered From';
      if (key === 'recovery.labels.overview') return 'Recovery overview';
      if (key === 'recovery.labels.stage') return 'Stage';
      if (key === 'recovery.labels.saved_at') return 'Saved';
      if (key === 'recovery.stage.transcribing') return 'Transcribing';
      if (key === 'recovery.stage.translating') return 'Translating';
      if (key === 'common.close') return 'Close';
      return key;
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../stores/projectStore', () => ({
  useProjectStore: (selector: any) => selector({
    projects: [
      { id: 'project-1', name: 'Team Sync' },
    ],
  }),
}));

vi.mock('../../stores/recoveryStore', () => ({
  useRecoveryStore: (selector: any) => selector(recoveryState),
}));

describe('RecoveryCenterModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    recoveryState.items = [
      {
        id: 'recovery-batch-1',
        filename: 'meeting.wav',
        filePath: 'C:\\watch\\meeting.wav',
        source: 'batch_import',
        resolution: 'pending',
        progress: 25,
        segments: [],
        projectId: 'project-1',
        historyId: 'history-1',
        lastKnownStage: 'transcribing',
        updatedAt: 100,
        hasSourceFile: true,
        canResume: true,
      },
      {
        id: 'recovery-automation-1',
        filename: 'automation.wav',
        filePath: 'C:\\watch\\automation.wav',
        source: 'automation',
        resolution: 'pending',
        progress: 55,
        segments: [],
        projectId: 'project-1',
        lastKnownStage: 'translating',
        updatedAt: 200,
        hasSourceFile: true,
        canResume: true,
        automationRuleId: 'rule-1',
        automationRuleName: 'Inbox Rule',
        sourceFingerprint: 'fp-1',
      },
    ];
    recoveryState.updatedAt = 300;
    recoveryState.isBusy = false;
    recoveryState.error = null;
  });

  it('renders grouped recovery overview and item sections', async () => {
    const { container } = render(<RecoveryCenterModal isOpen={true} onClose={vi.fn()} />);

    expect(screen.getByText('Interrupted Batch Recovery')).toBeDefined();
    expect(container.querySelector('.panel-modal-shell')).toBeTruthy();
    expect(container.querySelector('.panel-modal-header')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Close' })).toBeDefined();
    expect(screen.getAllByText('Batch Import').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Automation').length).toBeGreaterThan(0);
    expect(screen.getByText('meeting.wav')).toBeDefined();
    expect(screen.getByText('automation.wav')).toBeDefined();
  });

  it('triggers resume all and discard item actions', async () => {
    render(<RecoveryCenterModal isOpen={true} onClose={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Resume All' }));
    fireEvent.click(screen.getAllByRole('button', { name: 'Discard' })[0]);

    await waitFor(() => {
      expect(mockResumeAll).toHaveBeenCalledTimes(1);
      expect(mockDiscardItem).toHaveBeenCalledWith('recovery-batch-1');
    });
  });
});
