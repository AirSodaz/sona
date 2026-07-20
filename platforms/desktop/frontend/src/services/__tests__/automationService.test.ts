import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutomationProcessedEntry, AutomationRule } from '../../types/automation';

const testContext = vi.hoisted(() => ({
  automationLoadRepositoryStateMock: vi.fn(),
  automationPersistProcessedEntriesMock: vi.fn(),
  automationPersistProfilesMock: vi.fn(),
  automationPersistRepositoryStateMock: vi.fn(),
  automationPersistRulesMock: vi.fn(),
  automationValidateRuleActivationMock: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  BaseDirectory: {
    AppLocalData: 'AppLocalData',
  },
  exists: vi.fn().mockResolvedValue(true),
  mkdir: vi.fn().mockResolvedValue(undefined),
  readTextFile: vi.fn().mockResolvedValue('[]'),
  writeTextFile: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../tauri/automationRepository', () => ({
  automationLoadRepositoryState: testContext.automationLoadRepositoryStateMock,
  automationPersistProcessedEntries: testContext.automationPersistProcessedEntriesMock,
  automationPersistProfiles: testContext.automationPersistProfilesMock,
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
    projectId: 'project-1',
    presetId: 'meeting_notes',
    watchDirectory: 'C:\\watch',
    recursive: true,
    enabled: false,
    stageConfig: {
      autoPolish: false,
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

describe('automationService repository persistence', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads repository data through the native repository command', async () => {
    const rule = createRule();
    const processedEntry: AutomationProcessedEntry = {
      ruleId: rule.id,
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
    expect(testContext.automationPersistProcessedEntriesMock).toHaveBeenCalledWith([processedEntry]);
    expect(testContext.automationPersistRepositoryStateMock).toHaveBeenCalledWith([rule], [processedEntry], undefined);
  });

  it('delegates activation validation to the native repository command', async () => {
    const rule = createRule({ enabled: true });
    const config = { batchModelPath: 'C:\\models\\sensevoice' } as any;
    const project = { id: 'project-1', name: 'Team Sync' } as any;
    testContext.automationValidateRuleActivationMock.mockResolvedValue({ valid: true });

    await expect(validateAutomationRuleForActivation(rule, config, project)).resolves.toEqual({ valid: true });

    expect(testContext.automationValidateRuleActivationMock).toHaveBeenCalledWith(rule, config, project);
  });
});
