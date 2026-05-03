import type {
  DashboardLlmUsageStats,
  LlmUsageEventPayload,
} from '../types/dashboard';
import { getDashboardSnapshot } from './tauri/dashboard';
import { llmUsageEnsureStorage } from './tauri/llmUsage';

class LlmUsageService {
  async init(): Promise<void> {
    await llmUsageEnsureStorage();
  }

  async getStats(): Promise<DashboardLlmUsageStats> {
    return (await getDashboardSnapshot({ deep: false })).llmUsage;
  }

  async recordUsage(payload: LlmUsageEventPayload): Promise<void> {
    void payload;
    // Usage persistence now happens inside the Rust LLM command path.
  }
}

export const llmUsageService = new LlmUsageService();
