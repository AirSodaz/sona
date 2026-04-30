import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function replaceAutomationRuntimeRules<TResult = unknown>(
  rules: unknown[],
): Promise<TResult> {
  return invokeTauri<TResult>(TauriCommand.automation.replaceRuntimeRules, { rules });
}

export async function scanAutomationRuntimeRule(rule: unknown): Promise<void> {
  await invokeTauri<void>(TauriCommand.automation.scanRuntimeRule, { rule });
}

export async function collectAutomationRuntimeRulePaths<TResult = unknown>(
  rule: unknown,
  filePaths: string[],
): Promise<TResult> {
  return invokeTauri<TResult>(TauriCommand.automation.collectRuntimeRulePaths, {
    rule,
    filePaths,
  });
}
