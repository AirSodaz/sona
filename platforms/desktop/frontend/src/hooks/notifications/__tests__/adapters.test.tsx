import { describe, expect, it, vi } from 'vitest';
import type { TaskLedgerRecord } from '../../../types/taskLedger';
import { adaptModelDownloadEntry, adaptOnboardingEntry } from '../onboardingAdapter';
import { adaptTaskLedgerEntries } from '../taskLedgerAdapter';
import { adaptUpdateEntry } from '../updateAdapter';

const mockT = ((key: string, options?: { defaultValue?: string; stage?: string }) => {
  if (options?.defaultValue) return options.defaultValue;
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
