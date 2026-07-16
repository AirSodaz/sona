import type {
  AutomationRuntimePathCollectionResult,
  AutomationRuntimeReplaceResult,
  AutomationRuntimeRuleConfig,
} from '../../bindings';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function replaceAutomationRuntimeRules(
  rules: AutomationRuntimeRuleConfig[],
): Promise<AutomationRuntimeReplaceResult[]> {
  return invokeTauri(TauriCommand.automation.replaceRuntimeRules, { rules });
}

export async function scanAutomationRuntimeRule(rule: AutomationRuntimeRuleConfig): Promise<void> {
  await invokeTauri(TauriCommand.automation.scanRuntimeRule, { rule });
}

export async function collectAutomationRuntimeRulePaths(
  rule: AutomationRuntimeRuleConfig,
  filePaths: string[],
): Promise<AutomationRuntimePathCollectionResult[]> {
  return invokeTauri(TauriCommand.automation.collectRuntimeRulePaths, {
    rule,
    filePaths,
  });
}
