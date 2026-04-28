import { useAutomationStore } from '../../stores/automationStore';
import { useRecoveryStore } from '../../stores/recoveryStore';
import { logger } from '../../utils/logger';
import { healthCheckService } from '../healthCheckService';
import { llmUsageService } from '../llmUsageService';
import { voiceTypingService } from '../voiceTypingService';

async function runStartupStep(label: string, action: () => Promise<void>): Promise<void> {
  try {
    await action();
  } catch (error) {
    logger.error(`[Startup] Failed to ${label}:`, error);
  }
}

export async function startAppRuntimeServices(): Promise<void> {
  await runStartupStep('load recovery state', () => (
    useRecoveryStore.getState().loadRecovery()
  ));

  await runStartupStep('load automation runtime', () => (
    useAutomationStore.getState().loadAndStart()
  ));

  await runStartupStep('initialize llm usage service', () => (
    llmUsageService.init()
  ));

  await runStartupStep('initialize voice typing service', async () => {
    voiceTypingService.init();
  });

  await runStartupStep('run health check', () => (
    healthCheckService.runHealthCheck()
  ));
}
