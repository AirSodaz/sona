import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutomationProcessedEntry, AutomationRule } from '../../types/automation';
import type { AppConfig } from '../../types/config';
import type { ProjectRecord } from '../../types/project';

const testContext = vi.hoisted(() => ({
  automationLoadRepositoryStateMock: vi.fn(),
  automationPersistProcessedEntriesMock: vi.fn(),
  automationPersistRepositoryStateMock: vi.fn(),
  automationPersistRulesMock: vi.fn(),
  automationValidateRuleActivationMock: vi.fn(),
}));

vi.mock('../tauri/automationRepository', () => ({
  automationLoadRepositoryState: testContext.automationLoadRepositoryStateMock,
  automationPersistProcessedEntries: testContext.automationPersistProcessedEntriesMock,
  automationPersistRepositoryState: testContext.automationPersistRepositoryStateMock,
  automationPersistRules: testContext.automationPersistRulesMock,
  automationValidateRuleActivation: testContext.automationValidateRuleActivationMock,
}));

import {
  ensureAutomationStorage,
  loadAutomationProcessedEntries,
  loadAutomationRepositoryState,
  loadAutomationRules,
  saveAutomationProcessedEntries,
  saveAutomationRepositoryState,
  saveAutomationRules,
  validateAutomationRuleForActivation,
} from '../automation/automationService';

function createRule(overrides: Partial<AutomationRule> = {}): AutomationRule {
  return {
    id: 'rule-1',
    name: 'Meeting Inbox',
    saveHistory: true,
    projectId: 'project-1',
    watchDirectory: 'C:\\watch',
    recursive: true,
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

describe('automationService repository persistence', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads repository data through the native repository command', async () => {
    const rule = createRule();
    const processedEntry: AutomationProcessedEntry = {
      ruleId: rule.id,
      attempt: 1,
      filePath: 'C:\\watch\\meeting.wav',
      sourceFingerprint: 'fingerprint',
      size: 10,
      mtimeMs: 20,
      status: 'complete',
      processedAt: 30,
    };
    testContext.automationLoadRepositoryStateMock.mockResolvedValue({
      rules: [rule],
      processedEntries: [processedEntry],
    });

    await expect(loadAutomationRepositoryState()).resolves.toEqual({
      rules: [rule],
      processedEntries: [processedEntry],
    });
    await expect(loadAutomationRules()).resolves.toEqual([rule]);
    await expect(loadAutomationProcessedEntries()).resolves.toEqual([processedEntry]);
    await ensureAutomationStorage();

    expect(testContext.automationLoadRepositoryStateMock).toHaveBeenCalledTimes(4);
  });

  it('persists repository data through native repository commands', async () => {
    const rule = createRule();
    const processedEntry: AutomationProcessedEntry = {
      ruleId: rule.id,
      attempt: 1,
      filePath: 'C:\\watch\\meeting.wav',
      sourceFingerprint: 'fingerprint',
      size: 10,
      mtimeMs: 20,
      status: 'complete',
      processedAt: 30,
    };

    await saveAutomationRules([rule]);
    await saveAutomationProcessedEntries([processedEntry]);
    await saveAutomationRepositoryState([rule], [processedEntry]);

    expect(testContext.automationPersistRulesMock).toHaveBeenCalledWith([rule]);
    expect(testContext.automationPersistProcessedEntriesMock).toHaveBeenCalledWith([
      processedEntry,
    ]);
    expect(testContext.automationPersistRepositoryStateMock).toHaveBeenCalledWith(
      [rule],
      [processedEntry]
    );
  });

  it('delegates activation validation to the native repository command', async () => {
    const rule = createRule({ enabled: true });
    const config = {
      batchModelPath: 'C:\\models\\sensevoice',
    } as AppConfig;
    const projects = [{ id: 'project-1', name: 'Team Sync' } as ProjectRecord];
    testContext.automationValidateRuleActivationMock.mockResolvedValue({ valid: true });

    await expect(validateAutomationRuleForActivation(rule, config, projects)).resolves.toEqual({
      valid: true,
    });

    expect(testContext.automationValidateRuleActivationMock).toHaveBeenCalledWith(
      rule,
      config,
      projects
    );
  });
});
