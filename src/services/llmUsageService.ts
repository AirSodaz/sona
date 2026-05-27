import type {
  DashboardLlmUsageStats,
  LlmUsageEventPayload,
} from '../types/dashboard';
import { getDashboardSnapshot } from './tauri/dashboard';
import { llmUsageEnsureStorage } from './tauri/llmUsage';

export interface LlmUsageServicePorts {
  llmUsageEnsureStorage: typeof llmUsageEnsureStorage;
  getDashboardSnapshot: typeof getDashboardSnapshot;
}

export class LlmUsageService {
  constructor(private readonly ports: LlmUsageServicePorts) {}

  async init(): Promise<void> {
    await this.ports.llmUsageEnsureStorage();
  }

  async getStats(): Promise<DashboardLlmUsageStats> {
    return (await this.ports.getDashboardSnapshot({ deep: false })).llmUsage;
  }

  async recordUsage(payload: LlmUsageEventPayload): Promise<void> {
    void payload;
    // Usage persistence now happens inside the Rust LLM command path.
  }
}

export function createLlmUsageService(ports: LlmUsageServicePorts): LlmUsageService {
  return new LlmUsageService(ports);
}

export const llmUsageService = createLlmUsageService({
  llmUsageEnsureStorage,
  getDashboardSnapshot,
});
