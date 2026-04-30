import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function replaceAutomationRuntimeRules<TResult = unknown>(
  rules: unknown[],
): Promise<TResult> {
  return invokeTauri(TauriCommand.automation.replaceRuntimeRules, { rules }) as Promise<TResult>;
}

export async function scanAutomationRuntimeRule(rule: unknown): Promise<void> {
  await invokeTauri(TauriCommand.automation.scanRuntimeRule, { rule });
}

export async function collectAutomationRuntimeRulePaths<TResult = unknown>(
  rule: unknown,
  filePaths: string[],
): Promise<TResult> {
  return invokeTauri(TauriCommand.automation.collectRuntimeRulePaths, {
    rule,
    filePaths,
  }) as Promise<TResult>;
}
