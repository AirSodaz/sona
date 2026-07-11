import type { AppLogLevel } from '../types/config';

export const APP_LOG_LEVELS: readonly AppLogLevel[] = ['trace', 'debug', 'info', 'warn', 'error'];

const LOG_LEVEL_PRIORITY: Record<AppLogLevel, number> = {
  trace: 0,
  debug: 1,
  info: 2,
  warn: 3,
  error: 4,
};

export function normalizeLogLevel(value: unknown): AppLogLevel {
  return typeof value === 'string' && APP_LOG_LEVELS.includes(value as AppLogLevel)
    ? value as AppLogLevel
    : 'info';
}

export function shouldWriteLogLevel(messageLevel: AppLogLevel, configuredLevel: unknown): boolean {
  const normalizedConfiguredLevel = normalizeLogLevel(configuredLevel);
  return LOG_LEVEL_PRIORITY[messageLevel] >= LOG_LEVEL_PRIORITY[normalizedConfiguredLevel];
}
