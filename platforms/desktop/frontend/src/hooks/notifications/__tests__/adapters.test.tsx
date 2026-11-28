import { describe, expect, it, vi } from 'vitest';
import type { TaskLedgerRecord } from '../../../types/taskLedger';
import { adaptModelDownloadEntry, adaptOnboardingEntry } from '../onboardingAdapter';
import { adaptSpeakerDegradedEntry } from '../speakerDiarizationAdapter';
import { adaptTaskLedgerEntries, formatNotificationTimestamp } from '../taskLedgerAdapter';
import { adaptUpdateEntry } from '../updateAdapter';

function interpolate(template: string, options?: Record<string, unknown>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key) => String(options?.[key] ?? ''));
}

const mockT = ((
  key: string,
  options?: { defaultValue?: string; stage?: string } & Record<string, unknown>
) => {
  if (options?.defaultValue) return interpolate(options.defaultValue, options);
  if (options?.stage) return options.stage;
  return key;
}) as any;

describe('Notification Adapters Evaluation', () => {
  describe('adaptModelDownloadEntry', () => {
    const callbacks = {
      reopenOnboarding: vi.fn(),
      setModelDownloadIdle: vi.fn(),
    };

    it('returns null if onboarding modal is currently open', () => {
      const entry = adaptModelDownloadEntry(
        true,
        { status: 'downloading', progress: 50, error: '' },
        mockT,
        callbacks
      );
      expect(entry).toBeNull();
    });

    it('adapts downloading state with progress and no detail', () => {
      const entry = adaptModelDownloadEntry(
        false,
        { status: 'downloading', progress: 42, error: '' },
        mockT,
        callbacks
      );
      expect(entry).not.toBeNull();
      expect(entry?.title).toBe('Downloading models…');
      expect(entry?.body).toBe('Recommended models are being downloaded in the background.');
      expect(entry?.detail).toBeUndefined();
      expect(entry?.progress).toBe(42);
      expect(entry?.tone).toBe('accent');
    });

    it('adapts completed state without detail', () => {
      const entry = adaptModelDownloadEntry(
        false,
        { status: 'completed', progress: 100, error: '' },
        mockT,
        callbacks
      );
      expect(entry).not.toBeNull();
      expect(entry?.title).toBe('Models ready');
      expect(entry?.body).toBe('Local transcription models are installed and ready to use.');
      expect(entry?.detail).toBeUndefined();
      expect(entry?.tone).toBe('success');
    });

    it('adapts failed state with user body and technical error routed to detail', () => {
      const entry = adaptModelDownloadEntry(
        false,
        {
          status: 'failed',
          progress: 50,
          error: '  Failed to connect to huggingface.co: 504 Gateway Timeout \n ',
        },
        mockT,
        callbacks
      );
      expect(entry).not.toBeNull();
      expect(entry?.title).toBe('Model download failed');
      expect(entry?.body).toBe('Could not finish downloading the recommended models.');
      expect(entry?.detail).toBe('Failed to connect to huggingface.co: 504 Gateway Timeout');
      expect(entry?.tone).toBe('error');
      expect(entry?.actions.some((a) => a.id === 'retry')).toBe(true);
    });

    it('adapts failed state without error string with undefined detail', () => {
      const entry = adaptModelDownloadEntry(
        false,
        { status: 'failed', progress: 0, error: '' },
        mockT,
        callbacks
      );
      expect(entry).not.toBeNull();
      expect(entry?.title).toBe('Model download failed');
      expect(entry?.body).toBe('Could not finish downloading the recommended models.');
      expect(entry?.detail).toBeUndefined();
    });
  });
  const mockActionRegistry = {
    getOnboardingReminderActions: vi.fn(() => ({
      row: [],
      close: undefined,
    })),
    getLedgerTaskActions: vi.fn(() => ({
      row: [],
      close: undefined,
    })),
    getUpdateTaskActions: vi.fn(() => ({
      row: [],
      close: undefined,
    })),
  } as any;

  describe('adaptOnboardingEntry', () => {
    it('adapts onboarding reminder banner', () => {
      const entry = adaptOnboardingEntry(false, true, mockT, mockActionRegistry);
      expect(entry).not.toBeNull();
      expect(entry?.id).toBe('onboarding');
      expect(entry?.source).toBe('onboarding');
      expect(entry?.tone).toBe('accent');
      expect(entry?.detail).toBeUndefined();
    });

    it('returns null if onboarding is open or should not show', () => {
      expect(adaptOnboardingEntry(true, true, mockT, mockActionRegistry)).toBeNull();
      expect(adaptOnboardingEntry(false, false, mockT, mockActionRegistry)).toBeNull();
    });
  });

  describe('adaptTaskLedgerEntries', () => {
    it('adapts failed task and trims error message in detail', () => {
      const task: TaskLedgerRecord = {
        id: 'task-1',
        kind: 'batchImport',
        status: 'failed',
        title: 'interview.wav',
        progress: 0,
        createdAt: 1000,
        updatedAt: 2000,
        retryable: true,
        cancelable: false,
        recoverable: false,
        errorMessage: '  ConnectionRefusedError: [Errno 111]  \n\n ',
      };

      const [entry] = adaptTaskLedgerEntries([task], true, mockT, mockActionRegistry);
      expect(entry).toBeDefined();
      expect(entry.tone).toBe('error');
      expect(entry.detail).toBe('ConnectionRefusedError: [Errno 111]');
    });

    it('adapts cancel pending LLM task with hint in detail', () => {
      const task: TaskLedgerRecord = {
        id: 'llm-1',
        kind: 'llmPolish',
        status: 'cancelRequested',
        title: 'Polish note',
        progress: 50,
        createdAt: 1000,
        updatedAt: 2000,
        retryable: false,
        cancelable: false,
        recoverable: false,
      };

      const [entry] = adaptTaskLedgerEntries([task], true, mockT, mockActionRegistry);
      expect(entry).toBeDefined();
      expect(entry.detail).toBe('Stops after the current step and skips the final writeback.');
    });

    it('adapts unresumable recovery task with source missing error in detail', () => {
      const task: TaskLedgerRecord = {
        id: 'recovery-1',
        kind: 'recovery',
        status: 'recoverable',
        title: 'lost.wav',
        progress: 10,
        createdAt: 1000,
        updatedAt: 2000,
        retryable: false,
        cancelable: false,
        recoverable: false,
        errorMessage: 'Source file is missing.',
      };

      const [entry] = adaptTaskLedgerEntries([task], true, mockT, mockActionRegistry);
      expect(entry).toBeDefined();
      expect(entry.detail).toBe('Source file is missing.');
    });

    it('suppresses leaked terminal or stale stages on completed batch tasks', () => {
      const task: TaskLedgerRecord = {
        id: 'batch-done-1',
        kind: 'batchImport',
        status: 'succeeded',
        title: 'meeting.wav',
        progress: 100,
        createdAt: 1000,
        updatedAt: 2000,
        retryable: false,
        cancelable: false,
        recoverable: false,
        stage: 'completed',
      };

      const [entry] = adaptTaskLedgerEntries([task], true, mockT, mockActionRegistry);
      expect(entry.body).toBe('Batch import · Succeeded');
      expect(entry.detail).toBeNull();
    });

    it('localizes generic LLM task titles and includes target language for translate', () => {
      const tasks: TaskLedgerRecord[] = [
        {
          id: 'llm-polish-1',
          kind: 'llmPolish',
          status: 'running',
          title: 'LLM Polish',
          progress: 40,
          createdAt: 1000,
          updatedAt: 2000,
          retryable: true,
          cancelable: true,
          recoverable: false,
        },
        {
          id: 'llm-translate-1',
          kind: 'llmTranslate',
          status: 'succeeded',
          title: 'Translate',
          targetLanguage: 'ja',
          progress: 100,
          createdAt: 1000,
          updatedAt: 3000,
          retryable: false,
          cancelable: false,
          recoverable: false,
        },
      ];

      const entries = adaptTaskLedgerEntries(tasks, true, mockT, mockActionRegistry);
      const polishEntry = entries.find((e) => e.id === 'llm-polish-1');
      const translateEntry = entries.find((e) => e.id === 'llm-translate-1');

      expect(polishEntry?.title).toBe('LLM polish');
      expect(polishEntry?.body).toBe('Running');
      expect(translateEntry?.title).toBe('Translation (ja)');
      expect(translateEntry?.body).toBe('Succeeded');
    });

    it('aggregates 2 or more batchImport tasks per priority bucket and keeps 1 ungrouped', () => {
      const tasks: TaskLedgerRecord[] = [
        {
          id: 'batch-1',
          kind: 'batchImport',
          status: 'running',
          title: 'a.wav',
          progress: 60,
          createdAt: 1000,
          updatedAt: 3000,
          retryable: true,
          cancelable: true,
          recoverable: false,
        },
        {
          id: 'batch-2',
          kind: 'batchImport',
          status: 'pending',
          title: 'b.wav',
          progress: 0,
          createdAt: 1000,
          updatedAt: 2000,
          retryable: true,
          cancelable: true,
          recoverable: false,
        },
        {
          id: 'batch-3',
          kind: 'batchImport',
          status: 'succeeded',
          title: 'c.wav',
          progress: 100,
          createdAt: 1000,
          updatedAt: 1500,
          retryable: false,
          cancelable: false,
          recoverable: false,
        },
      ];

      const entries = adaptTaskLedgerEntries(tasks, true, mockT, mockActionRegistry);
      expect(entries).toHaveLength(2);

      const activeGroup = entries.find((e) => e.id === 'group-batchImport-active');
      expect(activeGroup).toBeDefined();
      expect(activeGroup?.title).toBe('Batch transcription · 2 files');
      expect(activeGroup?.progress).toBe(30);
      expect(activeGroup?.items).toHaveLength(2);

      const singleDone = entries.find((e) => e.id === 'batch-3');
      expect(singleDone).toBeDefined();
      expect(singleDone?.title).toBe('c.wav');
    });

    it('aggregates automation tasks per ruleId with resolved rule name', () => {
      const tasks: TaskLedgerRecord[] = [
        {
          id: 'batch-auto-1',
          kind: 'automation',
          status: 'failed',
          title: 'one.wav',
          automationRuleId: 'rule-podcast',
          errorMessage: 'ASR error',
          progress: 0,
          createdAt: 1000,
          updatedAt: 2000,
          retryable: true,
          cancelable: false,
          recoverable: false,
        },
        {
          id: 'batch-auto-2',
          kind: 'automation',
          status: 'failed',
          title: 'two.wav',
          automationRuleId: 'rule-podcast',
          errorMessage: 'Timeout',
          progress: 0,
          createdAt: 1000,
          updatedAt: 3000,
          retryable: true,
          cancelable: false,
          recoverable: false,
        },
      ];

      const entries = adaptTaskLedgerEntries(tasks, true, mockT, mockActionRegistry, {
        ruleNameById: { 'rule-podcast': 'Podcast Watch' },
      });

      expect(entries).toHaveLength(1);
      expect(entries[0].id).toBe('group-automation-rule-podcast-action');
      expect(entries[0].title).toBe('Podcast Watch · 2 files failed');
      expect(entries[0].items).toHaveLength(2);
    });
  });

  describe('formatNotificationTimestamp', () => {
    it('formats recent real timestamps and skips synthetic small test timestamps', () => {
      const now = 1_760_000_000_000;
      expect(formatNotificationTimestamp(1000, now, mockT)).toBeNull();
      expect(formatNotificationTimestamp(now - 15_000, now, mockT)).toBe('Just now');
      expect(formatNotificationTimestamp(now - 5 * 60_000, now, mockT)).toBe('5m ago');
      expect(formatNotificationTimestamp(now - 2 * 3_600_000, now, mockT)).toBe('2h ago');
    });
  });

  describe('adaptSpeakerDegradedEntry & pinnedRank', () => {
    it('shows count when degradedCount > 1 and assigns deterministic pinnedRank', () => {
      const single = adaptSpeakerDegradedEntry(true, mockT, vi.fn(), vi.fn(), 1);
      const multiple = adaptSpeakerDegradedEntry(true, mockT, vi.fn(), vi.fn(), 4);
      const onboarding = adaptOnboardingEntry(false, true, mockT, mockActionRegistry);
      const update = adaptUpdateEntry(
        'available',
        { version: '1.2.0', currentVersion: '1.1.0', body: 'Notes' } as any,
        0,
        true,
        mockT,
        mockActionRegistry
      );

      expect(multiple?.body).toContain('4 tasks');
      expect(single?.pinnedRank).toBe(1);
      expect(update?.pinnedRank).toBe(3);
      expect(onboarding?.pinnedRank).toBe(4);
    });
  });

  describe('adaptUpdateEntry', () => {
    it('adapts available update with changelog body and update-body class', () => {
      const entry = adaptUpdateEntry(
        'available',
        {
          version: '1.2.0',
          currentVersion: '1.1.0',
          body: '## Changes\n- New notification collapse\n- Performance fixes',
          date: '2026-10-08',
        } as any,
        0,
        true,
        mockT,
        mockActionRegistry
      );

      expect(entry).not.toBeNull();
      expect(entry?.body).toBe('## Changes\n- New notification collapse\n- Performance fixes');
      expect(entry?.bodyClassName).toBe('notification-center-update-body');
      expect(entry?.detail).toBeUndefined();
    });
  });
});
