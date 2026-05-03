import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function llmUsageEnsureStorage(): Promise<void> {
  await invokeTauri(TauriCommand.llmUsage.ensureStorage);
}

export async function llmUsageReadRaw(): Promise<string> {
  return invokeTauri(TauriCommand.llmUsage.readRaw);
}

export async function llmUsageReplaceRaw(content: string): Promise<void> {
  await invokeTauri(TauriCommand.llmUsage.replaceRaw, { content });
}
