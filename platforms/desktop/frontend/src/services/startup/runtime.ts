import { useAutomationStore } from '../../stores/automationStore';
import { useRecoveryStore } from '../../stores/recoveryStore';
import { useTaskLedgerStore } from '../../stores/taskLedgerStore';
import { logger } from '../../utils/logger';
import { healthCheckService } from '../healthCheckService';
import { runHistoryAudioCleanupForCurrentConfig } from '../historyAudioCleanupService';
import { voiceTypingService } from '../voiceTypingService';
import { syncRuntimeService } from '../syncRuntimeService';

async function runStartupStep(label: string, action: () => Promise<void>): Promise<void> {
  try {
    await action();
  } catch (error) {
    logger.error(`[Startup] Failed to ${label}:`, error);
  }
}

export async function startAppRuntimeServices(): Promise<void> {
  await runStartupStep('load task ledger', () => (
    useTaskLedgerStore.getState().loadTasks()
  ));

  await runStartupStep('load recovery state', () => (
    useRecoveryStore.getState().loadRecovery()
  ));

  await runStartupStep('load automation runtime', () => (
    useAutomationStore.getState().loadAndStart()
  ));

  await runStartupStep('initialize voice typing service', async () => {
    voiceTypingService.init();
  });

  await runStartupStep('initialize sync runtime', async () => {
    syncRuntimeService.init();
  });

  await runStartupStep('run health check', () => (
    healthCheckService.runHealthCheck()
  ));

  await runStartupStep('clean up history audio', async () => {
    await runHistoryAudioCleanupForCurrentConfig();
  });
}
